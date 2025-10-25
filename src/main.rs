mod config;
mod helpers;
mod http;
mod routes;

use std::{fs, io::ErrorKind, time::Duration};

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
    }
}

async fn run_server() {
    info!("Running {} server version {}", NAME, VERSION);

    let home_dir = match dirs::home_dir() {
        Some(path) => path,
        None => {
            error!("Unable to get your home dir!");
            std::process::exit(1);
        }
    };

    let config_path = home_dir.join(".timesplit.toml");

    if !config_path.exists() {
        let default_config = AppConfig::default();

        match toml::to_string_pretty(&default_config) {
            Ok(serialized) => {
                if let Err(err) = fs::write(&config_path, serialized) {
                    warn!(
                        "Failed to write default config to {}: {}",
                        config_path.display(),
                        err
                    );
                } else {
                    info!("Created default config at '{}'", config_path.display());
                }
            }
            Err(err) => warn!("Failed to serialize default config: {}", err),
        }
    }

    let settings: AppConfig = match Config::builder()
        .add_source(File::from(config_path.clone()).required(false))
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
    };

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
