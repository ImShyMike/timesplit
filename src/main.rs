mod config;
mod helpers;
mod http;
mod routes;

use std::{
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
    time::Duration,
};

use ::config::{Config, Environment, File};
use axum::body::Body;
use axum::http::Request as HttpRequest;
use axum::middleware::Next;
use axum::{
    Router,
    response::{IntoResponse, Redirect},
    routing::{get, post},
};
use clap::{Parser, Subcommand};
use reqwest::Client;

use tower_http::trace::TraceLayer;
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;

use crate::config::AppConfig;
use crate::routes::{heartbeats_handler, statusbar_handler};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const NAME: &str = env!("CARGO_PKG_NAME");

async fn request_logger(req: HttpRequest<Body>, next: Next) -> impl IntoResponse {
    info!("{} {}", &req.method(), &req.uri());

    next.run(req).await
}

#[derive(Clone)]
pub struct AppState {
    client: Client,
    servers: Vec<(String, String)>,
}

#[derive(Parser, Debug)]
#[command(name = NAME, version = VERSION, about = "timesplit - wakatime relay", arg_required_else_help = true)]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Start the HTTP server
    Run,
    /// Change the configuration
    #[command(subcommand, arg_required_else_help = true)]
    Config(ConfigCommand),
}

#[derive(Subcommand, Debug)]
enum ConfigCommand {
    /// List configured servers
    List,
    /// Add a new server
    #[command(arg_required_else_help = true)]
    Add {
        /// Base URL of the server
        url: String,
        /// API key for the server
        key: String,
        /// Insert as the main server instead of a clone
        #[arg(long)]
        main: bool,
    },
    /// Remove a server by its index (use `config list` to find indexes)
    #[command(arg_required_else_help = true)]
    Remove {
        /// Index of the server to remove
        index: usize,
    },
    /// Make a server the main server by its index
    #[command(arg_required_else_help = true)]
    Main {
        /// Index of the server to make main
        index: usize,
    },
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let args = Args::parse();

    match args.command {
        Command::Run => run_server().await,
        Command::Config(action) => change_config(action),
    }
}

fn save_config(config: &AppConfig, config_path: &Path) {
    match toml::to_string_pretty(&config) {
        Ok(serialized) => {
            if let Err(err) = fs::write(config_path, serialized) {
                error!(
                    "Failed to write config to {}: {}",
                    config_path.display(),
                    err
                );
                std::process::exit(1);
            } else {
                info!("Configuration saved to '{}'", config_path.display());
            }
        }
        Err(err) => {
            error!("Failed to serialize config: {}", err);
            std::process::exit(1);
        }
    }
}

fn change_config(action: ConfigCommand) {
    let config_path = get_config_path();

    ensure_config_file_exists(&config_path);

    let mut settings = load_config(&config_path);

    match action {
        ConfigCommand::List => list_servers(&settings),
        ConfigCommand::Add { url, key, main } => {
            add_server(&mut settings, url, key, main);
            save_config(&settings, &config_path);
        }
        ConfigCommand::Remove { index } => {
            remove_server(&mut settings, index);
            save_config(&settings, &config_path);
        }
        ConfigCommand::Main { index } => {
            set_main_server(&mut settings, index);
            save_config(&settings, &config_path);
        }
    }
}

fn list_servers(settings: &AppConfig) {
    if settings.servers.is_empty() {
        println!("No servers configured.");
        return;
    }

    println!("Configured servers:");
    for (idx, (url, key)) in settings.servers.iter().enumerate() {
        let role = if idx == 0 { " (main)" } else { "" };
        println!("{}: {}{} [key {}]", idx, url, role, mask_key(key));
    }
}

fn add_server(settings: &mut AppConfig, url: String, key: String, main: bool) {
    let trimmed_url = url.trim();
    if trimmed_url.is_empty() {
        error!("Server URL cannot be empty.");
        std::process::exit(1);
    }

    if key.trim().is_empty() {
        error!("Server key cannot be empty.");
        std::process::exit(1);
    }

    if main {
        settings.servers.insert(0, (trimmed_url.to_string(), key));
        info!("Added server '{}' as the main server", trimmed_url);
    } else {
        settings.servers.push((trimmed_url.to_string(), key));
        info!("Added server '{}'", trimmed_url);
    }
}

fn remove_server(settings: &mut AppConfig, index: usize) {
    if settings.servers.is_empty() {
        error!("No servers to remove.");
        std::process::exit(1);
    }

    if index >= settings.servers.len() {
        error!("Server index {} is out of range.", index);
        std::process::exit(1);
    }

    if settings.servers.len() == 1 {
        error!("Cannot remove the last remaining server. Add a new server first.");
        std::process::exit(1);
    }

    let removed = settings.servers.remove(index);
    info!("Removed server '{}'", removed.0);
}

fn set_main_server(settings: &mut AppConfig, index: usize) {
    if settings.servers.is_empty() {
        error!("No servers to set as main.");
        std::process::exit(1);
    }

    if index >= settings.servers.len() {
        error!("Server index {} is out of range.", index);
        std::process::exit(1);
    }

    if index == 0 {
        info!(
            "Server '{}' is already the main server.",
            settings.servers[0].0
        );
        return;
    }

    let main_server = settings.servers.remove(index);
    settings.servers.insert(0, main_server.clone());
    info!("Set server '{}' as the main server.", main_server.0);
}

fn mask_key(key: &str) -> String {
    if key.len() <= 8 {
        "*".repeat(key.len())
    } else {
        format!("{}...{}", &key[..4], &key[key.len() - 4..])
    }
}

fn get_config_path() -> PathBuf {
    let home_dir = match dirs::home_dir() {
        Some(path) => path,
        None => {
            error!("Unable to get your home dir!");
            std::process::exit(1);
        }
    };

    home_dir.join(".timesplit.toml")
}

fn ensure_config_file_exists(config_path: &Path) {
    if !config_path.exists() {
        let default_config = AppConfig::default();
        save_config(&default_config, config_path);
    }
}

fn load_config(config_path: &Path) -> AppConfig {
    match Config::builder()
        .add_source(File::from(config_path).required(false))
        .add_source(Environment::with_prefix("TIMESPLIT"))
        .build()
    {
        Ok(config) => match config.try_deserialize() {
            Ok(settings) => settings,
            Err(err) => {
                warn!("Failed to deserialize configuration: {}", err);
                AppConfig::default()
            }
        },
        Err(err) => {
            warn!("Failed to load configuration: {}", err);
            AppConfig::default()
        }
    }
}

async fn run_server() {
    info!("Running {} server version {}", NAME, VERSION);

    let config_path = get_config_path();

    ensure_config_file_exists(&config_path);

    let settings = load_config(&config_path);

    if settings.servers.is_empty() {
        error!("No servers configured! Please add at least one server to your configuration.");
        std::process::exit(1);
    } else if settings.servers[0].0 == "https://example.com" {
        error!(
            "The main server URL is set to the default 'example.com'. Please update your configuration."
        );
        std::process::exit(1);
    }

    let client = Client::builder()
        .timeout(Duration::from_secs(settings.timeout))
        .build()
        .unwrap();

    let state = AppState {
        client,
        servers: settings.servers.clone(),
    };

    let fallback_url = settings.servers[0].0.clone();

    let app = Router::new()
        .nest(
            "/users/current",
            Router::new()
                .route("/statusbar/today", get(statusbar_handler))
                .route("/status_bar/today", get(statusbar_handler))
                .route("/heartbeats", post(heartbeats_handler))
                .route("/heartbeats.bulk", post(heartbeats_handler)),
        )
        .with_state(state)
        .fallback(move || async move { Redirect::to(fallback_url.as_str()) })
        .layer(axum::middleware::from_fn(request_logger))
        .layer(TraceLayer::new_for_http());

    info!("Listening on http://{}", settings.host);

    let listener = match tokio::net::TcpListener::bind(&settings.host).await {
        Ok(listener) => listener,
        Err(err) => {
            match err.kind() {
                ErrorKind::AddrInUse => error!("The address {} is already in use.", settings.host),
                _ => error!("Failed to bind to {}: {}", settings.host, err),
            }

            std::process::exit(1);
        }
    };

    if let Err(err) = axum::serve(listener, app).await {
        error!("Server error: {}", err);
    }
}
