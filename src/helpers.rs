use axum::http::{HeaderMap, Uri};

use crate::{NAME, VERSION};

#[inline(always)]
pub fn get_modified_user_agent(headers: &HeaderMap) -> String {
    let user_agent = headers
        .get("User-Agent")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("Unknown");
    format!("{user_agent} {}/{}", NAME, VERSION)
}

#[inline(always)]
pub fn get_formatted_url(uri: &Uri, base_url: &str) -> String {
    match uri.query() {
        Some(query) => format!("{}/users/current{}?{}", base_url, uri.path(), query),
        None => format!("{}/users/current{}", base_url, uri.path()),
    }
}
