use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Clone)]
pub struct AppConfig {
    pub host: String,
    pub timeout: u64,
    pub servers: Vec<(String, String)>,
}

impl AppConfig {
    pub fn default() -> Self {
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
