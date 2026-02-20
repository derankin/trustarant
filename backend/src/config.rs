use std::env;

#[derive(Clone, Debug)]
pub struct Settings {
    pub host: String,
    pub port: u16,
    pub cors_origin: String,
    pub ingestion_interval_hours: u64,
}

impl Settings {
    pub fn from_env() -> Self {
        Self {
            host: env::var("TRUSTARANT_HOST").unwrap_or_else(|_| "0.0.0.0".into()),
            port: env::var("TRUSTARANT_PORT")
                .ok()
                .and_then(|value| value.parse::<u16>().ok())
                .unwrap_or(8080),
            cors_origin: env::var("TRUSTARANT_CORS_ORIGIN")
                .unwrap_or_else(|_| "http://localhost:5173".into()),
            ingestion_interval_hours: env::var("TRUSTARANT_INGESTION_INTERVAL_HOURS")
                .ok()
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or(24),
        }
    }
}
