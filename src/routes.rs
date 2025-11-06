use crate::helpers::{get_formatted_url, get_modified_user_agent};
use crate::http::request;
use axum::extract::State;
use axum::http::Uri;
use axum::{
    Json,
    http::{HeaderMap, StatusCode, method::Method},
    response::{IntoResponse, Response},
};
use serde_json::Value;
use tracing::{debug, error, warn};

use crate::AppState;

/// Process main server response, handling errors and extracting JSON
async fn process_main_server_response(
    response: reqwest::Response,
    url: &str,
) -> Result<(StatusCode, Value), Response> {
    let status = response.status();

    if !status.is_success() {
        let body_bytes = response.bytes().await.map_err(|e| {
            error!(
                "Failed to read main server response body from {}: {}",
                url, e
            );
            (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response()
        })?;
        let body_text = String::from_utf8_lossy(&body_bytes);
        error!(
            "Main server returned non-success status {} for {}: {}",
            status, url, body_text
        );
        return Err((StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response());
    }

    let json_data = response.json::<Value>().await.map_err(|e| {
        error!(
            "Failed to parse JSON from main server {}: {}",
            url, e
        );
        (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response()
    })?;

    Ok((status, json_data))
}

pub async fn statusbar_handler(
    state: State<AppState>,
    headers: HeaderMap,
    uri: Uri,
) -> Result<Json<Value>, Response> {
    let new_user_agent = get_modified_user_agent(&headers);
    let formatted_url = get_formatted_url(&uri, &state.servers[0].0);

    debug!("Getting status bar from main server... ({})", formatted_url);
    let main_response = request(
        Method::GET,
        &formatted_url,
        &state.client,
        headers.clone(),
        &new_user_agent,
        &state.servers[0].1,
        None,
    )
    .await
    .map_err(|err| {
        error!(
            "Failed to get response from main server {}: {}",
            formatted_url, err
        );
        (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response()
    })?;

    let (_status, json_data) = process_main_server_response(main_response, &formatted_url).await?;

    Ok(Json(json_data))
}

pub async fn heartbeats_handler(
    state: State<AppState>,
    headers: HeaderMap,
    uri: Uri,
    Json(json_data): Json<Value>,
) -> Result<Response, Response> {
    let new_user_agent = get_modified_user_agent(&headers);

    debug!(payload = ?json_data, "Received heartbeat payload");

    let formatted_url = get_formatted_url(&uri, &state.servers[0].0);
    let main_response = request(
        Method::POST,
        &formatted_url,
        &state.client,
        headers.clone(),
        &new_user_agent,
        &state.servers[0].1,
        Some(&json_data),
    )
    .await;

    debug!("Response received from main server, requesting clones...");

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
            Some(&json_data),
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

    let (status, json_data) = process_main_server_response(main_response, &formatted_url).await?;

    Ok((status, Json(json_data)).into_response())
}
