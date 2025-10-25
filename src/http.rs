use axum::http::{
    HeaderMap,
    header::{AUTHORIZATION, CONTENT_ENCODING, CONTENT_LENGTH, HOST},
    method::Method,
};
use base64::Engine;
use reqwest::Client;
use serde_json::Value;
use std::future::Future;
use std::result;

pub fn request(
    method: Method,
    url: &str,
    client: &Client,
    mut headers: HeaderMap,
    user_agent: &str,
    key: &str,
    json_data: Option<&Value>,
) -> impl Future<Output = result::Result<reqwest::Response, reqwest::Error>> {
    let mut request_builder = match method {
        Method::GET => client.get(url),
        Method::POST => client.post(url),
        _ => unreachable!(),
    };

    headers.remove(CONTENT_ENCODING);
    headers.remove(CONTENT_LENGTH);
    headers.remove(HOST);
    headers.remove(AUTHORIZATION);

    let base64_key = base64::engine::general_purpose::STANDARD.encode(key);

    if let Some(json_data) = json_data {
        request_builder = request_builder.json(&json_data);
    }

    request_builder
        .headers(headers)
        .header("User-Agent", user_agent)
        .header("Authorization", format!("Basic {}", base64_key))
        .send()
}
