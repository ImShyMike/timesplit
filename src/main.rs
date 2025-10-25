use std::{fs, result, time::Duration};

use axum::body::{Body, to_bytes};
use axum::extract::State;
use axum::http::Request as HttpRequest;
use axum::middleware::Next;
use axum::{
    Json, Router,
    extract::Request,
    http::{
        HeaderMap, StatusCode,
        header::{AUTHORIZATION, CONTENT_ENCODING, CONTENT_LENGTH, HOST},
        method::Method,
    },
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
};
use base64::Engine;
use bytes::Bytes;
use config::Config;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::future::Future;
use tower_http::trace::TraceLayer;
use tracing::{debug, error, info, warn};
use tracing_subscriber::EnvFilter;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const NAME: &str = env!("CARGO_PKG_NAME");

#[derive(Deserialize, Serialize, Clone)]
struct AppConfig {
    host: String,
    timeout: u64,
    servers: Vec<(String, String)>,
}

impl AppConfig {
    fn default() -> Self {
        Self {
            host: String::from("127.0.0.1:25893"),
            timeout: 10,
            servers: vec![(
                "https://example.com".to_string(),
                "key_goes_here".to_string(),
            )],
        }
    }
}

fn get_modified_user_agent(user_agent: &str) -> String {
    format!("{user_agent} {}/{}", NAME, VERSION)
}

fn request(
    method: Method,
    url: &str,
    client: &Client,
    mut headers: HeaderMap,
    user_agent: &str,
    key: &str,
    body: Option<Bytes>,
) -> impl Future<Output = result::Result<reqwest::Response, reqwest::Error>> {
    let mut request_builder = match method {
        Method::GET => client.get(url),
        Method::POST => client.post(url),
        _ => unreachable!(),
    };

    // Drop hop-by-hop headers so reqwest can set appropriate values for the upstream request.
    headers.remove(CONTENT_ENCODING);
    headers.remove(CONTENT_LENGTH);
    headers.remove(HOST);
    headers.remove(AUTHORIZATION);

    let base64_key = base64::engine::general_purpose::STANDARD.encode(key);

    if let Some(body) = body {
        request_builder = request_builder.body(body);
    }

    request_builder
        .headers(headers)
        .header("User-Agent", user_agent)
        .header("Authorization", format!("Basic {}", base64_key))
        .send()
}

async fn statusbar_handler(
    state: State<AppState>,
    headers: HeaderMap,
    incoming_request: Request,
) -> Result<Json<Value>, Response> {
    let new_user_agent = get_modified_user_agent(
        headers
            .get("User-Agent")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("Unknown"),
    );

    let uri = incoming_request.uri();
    let formatted_url = match uri.query() {
        Some(query) => format!(
            "{}/users/current{}?{}",
            state.servers[0].0,
            uri.path(),
            query
        ),
        None => format!("{}/users/current{}", state.servers[0].0, uri.path()),
    };

    debug!("Getting status bar from main server... ({})", formatted_url);
    let main_response = match request(
        Method::GET,
        &formatted_url,
        &state.client,
        headers.clone(),
        &new_user_agent,
        &state.servers[0].1,
        None,
    )
    .await
    {
        Ok(response) => response,
        Err(err) => {
            error!(
                "Failed to get response from main server {}: {}",
                formatted_url, err
            );
            return Err(
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response()
            );
        }
    };

    let status = main_response.status();
    let body_bytes = main_response.bytes().await.map_err(|e| {
        error!(
            "Failed to read main server response body from {}: {}",
            formatted_url, e
        );
        (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response()
    })?;

    if !status.is_success() {
        let body_text = String::from_utf8_lossy(&body_bytes);
        error!(
            "Main server returned non-success status {} for {}: {}",
            status, formatted_url, body_text
        );
        return Err((StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response());
    }

    debug!("Processing main server response...");
    let value = serde_json::from_slice::<Value>(&body_bytes).map_err(|e| {
        error!(
            "Failed to parse main server response JSON from {}: {} (payload: {})",
            formatted_url,
            e,
            String::from_utf8_lossy(&body_bytes)
        );
        (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response()
    })?;

    Ok(Json(value))
}

async fn heartbeats_handler(
    state: State<AppState>,
    headers: HeaderMap,
    incoming_request: Request,
) -> Result<Response, Response> {
    let new_user_agent = get_modified_user_agent(
        headers
            .get("User-Agent")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("Unknown"),
    );

    let uri = incoming_request.uri().clone();
    let (_, body) = incoming_request.into_parts();
    let request_body = to_bytes(body, usize::MAX).await.map_err(|e| {
        error!("Failed to read incoming request body: {}", e);
        (StatusCode::BAD_REQUEST, "Invalid request body").into_response()
    })?;

    let heartbeat_json: Value = serde_json::from_slice(&request_body).map_err(|e| {
        error!("Failed to parse incoming heartbeat JSON payload: {}", e);
        (StatusCode::BAD_REQUEST, "Invalid JSON payload").into_response()
    })?;

    debug!(payload = ?heartbeat_json, "Received heartbeat payload");

    let forwarded_body = request_body.clone();

    let formatted_url = format!("{}/users/current{}", state.servers[0].0, uri.path());

    debug!(
        size_bytes = forwarded_body.len(),
        "Validated heartbeat payload size"
    );

    let main_response = request(
        Method::POST,
        &formatted_url,
        &state.client,
        headers.clone(),
        &new_user_agent,
        &state.servers[0].1,
        Some(forwarded_body.clone()),
    )
    .await;

    debug!("Response received (or failed) from main server, requesting clones...");

    for pair in &state.servers[1..] {
        let url = &pair.0;
        let key = &pair.1;

        let formatted_url = format!("{}/users/current{}", url, uri.path());

        match request(
            Method::POST,
            &formatted_url,
            &state.client,
            headers.clone(),
            &new_user_agent,
            key,
            Some(forwarded_body.clone()),
        )
        .await
        {
            Ok(response) => {
                let status = response.status();
                debug!(url = %formatted_url, status = %status, "Clone server heartbeat response status");
                if !status.is_success() {
                    match response.text().await {
                        Ok(body) => warn!(
                            "Clone server returned non-success status {} for {}: {}",
                            status, formatted_url, body
                        ),
                        Err(err) => warn!(
                            "Clone server returned non-success status {} for {} but body read failed: {}",
                            status, formatted_url, err
                        ),
                    }
                }
            }
            Err(err) => {
                warn!(
                    "Failed to get response from clone server {}: {}",
                    formatted_url, err
                );
            }
        }
    }

    let main_response = match main_response {
        Ok(response) => response,
        Err(err) => {
            error!(
                "Failed to get response from main server {}: {}",
                formatted_url, err
            );
            return Err(
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response()
            );
        }
    };

    let status = main_response.status();
    let body_bytes = main_response.bytes().await.map_err(|e| {
        error!(
            "Failed to read main server response body from {}: {}",
            formatted_url, e
        );
        (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response()
    })?;

    if !status.is_success() {
        let body_text = String::from_utf8_lossy(&body_bytes);
        error!(
            "Main server returned non-success status {} for {}: {}",
            status, formatted_url, body_text
        );
        return Err((StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response());
    }

    debug!("Processing main server response...");
    let value = serde_json::from_slice::<Value>(&body_bytes).map_err(|e| {
        error!(
            "Failed to parse main server response JSON from {}: {} (payload: {})",
            formatted_url,
            e,
            String::from_utf8_lossy(&body_bytes)
        );
        (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response()
    })?;

    debug!(status = %status, payload = ?value, "Main server heartbeat response JSON");

    Ok((status, Json(value)).into_response())
}

async fn request_logger(req: HttpRequest<Body>, next: Next) -> impl IntoResponse {
    info!("{} {}", &req.method(), &req.uri());

    next.run(req).await
}

#[derive(Clone)]
struct AppState {
    client: Client,
    servers: Vec<(String, String)>,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    info!("Running {} version {}", NAME, VERSION);

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
        .add_source(config::File::from(config_path.clone()).required(false))
        .add_source(config::Environment::with_prefix("TIMESPLIT"))
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

    // listening locally on port 25892
    let listener = tokio::net::TcpListener::bind(settings.host).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
