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
use ini::Ini;
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
#[command(name = NAME, version = VERSION, about = "timesplit - the easy to use wakatime relay", arg_required_else_help = true)]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Start the HTTP server
    Run,
    /// Setup WakaTime config
    Setup,
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
        Command::Setup => setup_config(),
        Command::Config(action) => change_config(action),
    }
}

fn setup_config() {
    let home_path = match dirs::home_dir() {
        Some(path) => path,
        None => {
            error!("Unable to get your home dir!");
            std::process::exit(1);
        }
    };

    let wakatime_config_path = home_path.join(".wakatime.cfg");
    let timesplit_config_path = get_config_path();

    // backup if the wakatime config exists
    if wakatime_config_path.exists() {
        fs::copy(&wakatime_config_path, home_path.join(".wakatime.cfg.bak")).unwrap_or_else(
            |err| {
                error!("Failed to backup WakaTime config: {}", err);
                std::process::exit(1);
            },
        );
        info!("Backed up existing WakaTime config to .wakatime.cfg.bak");
    }

    ensure_config_file_exists(&timesplit_config_path);

    let mut settings = load_config(&timesplit_config_path);

    sync_main_server_from_wakatime(&mut settings, &wakatime_config_path, &timesplit_config_path);

    // write new wakatime config
    let wakatime_config_content = format!(
        "[settings]\napi_url = \"http://{}\"\napi_key = \"9542e1b0-59c3-4238-b51a-e4e4757038ad\"\nheartbeat_rate_limit_seconds = 30\n", // dummy key
        settings.host
    );

    match fs::write(&wakatime_config_path, wakatime_config_content) {
        Ok(_) => info!(
            "WakaTime config written to '{}'",
            wakatime_config_path.display()
        ),
        Err(err) => {
            error!(
                "Failed to write WakaTime config to {}: {}",
                wakatime_config_path.display(),
                err
            );
            std::process::exit(1);
        }
    }
}

fn sync_main_server_from_wakatime(
    settings: &mut AppConfig,
    wakatime_config_path: &Path,
    timesplit_config_path: &Path,
) {
    let Some((legacy_url, legacy_key)) = read_wakatime_settings(wakatime_config_path) else {
        return;
    };

    let timesplit_host_url = format!("http://{}", settings.host);
    if legacy_url == timesplit_host_url {
        info!(
            "Existing WakaTime config already points to the timesplit host; skipping server update."
        );
        return;
    }

    if legacy_url.is_empty() || legacy_key.is_empty() {
        warn!("Skipping server sync because the stored WakaTime url/key is empty.");
        return;
    }

    if settings.servers.is_empty() {
        settings
            .servers
            .push((legacy_url.clone(), legacy_key.clone()));
    } else {
        settings.servers[0] = (legacy_url.clone(), legacy_key.clone());
    }

    save_config(settings, timesplit_config_path);
    info!(
        "Primary server updated to '{}' based on the existing WakaTime configuration.",
        legacy_url
    );
}

fn read_wakatime_settings(path: &Path) -> Option<(String, String)> {
    if !path.exists() {
        return None;
    }

    let wakatime_config = Ini::load_from_file(path).unwrap_or_else(|err| {
        warn!(
            "Unable to parse WakaTime config at '{}': {}",
            path.display(),
            err
        );
        Ini::new()
    });

    let config_section = wakatime_config
        .section(Some("settings"))
        .unwrap_or_else(|| {
            error!(
                "No [settings] section found in WakaTime config at '{}'",
                path.display()
            );
            std::process::exit(1);
        });

    let api_url = config_section.get("api_url");
    let api_key = config_section.get("api_key");

    match (api_url, api_key) {
        (Some(url), Some(key)) => Some((url.to_string(), key.to_string())),
        _ => None,
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
    validate_server_index(settings, index, "remove");

    if settings.servers.len() == 1 {
        error!("Cannot remove the last remaining server. Add a new server first.");
        std::process::exit(1);
    }

    let removed = settings.servers.remove(index);
    info!("Removed server '{}'", removed.0);
}

fn set_main_server(settings: &mut AppConfig, index: usize) {
    validate_server_index(settings, index, "set as main");

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

/// Validate server index, checking for empty list and out of range
fn validate_server_index(settings: &AppConfig, index: usize, operation: &str) {
    if settings.servers.is_empty() {
        error!("No servers to {}.", operation);
        std::process::exit(1);
    }

    if index >= settings.servers.len() {
        error!("Server index {} is out of range.", index);
        std::process::exit(1);
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
