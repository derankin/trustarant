use std::env;

#[derive(Clone, Debug)]
pub struct Settings {
    pub host: String,
    pub port: u16,
    pub cors_origin: String,
    pub ingestion_interval_hours: u64,
    pub run_mode: RunMode,
    pub database_url: Option<String>,
    pub enable_background_ingestion: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RunMode {
    Api,
    Worker,
    RefreshOnce,
}

impl Settings {
    pub fn from_env() -> Self {
        let run_mode = match env::var("TRUSTARANT_RUN_MODE")
            .unwrap_or_else(|_| "api".into())
            .to_ascii_lowercase()
            .as_str()
        {
            "worker" => RunMode::Worker,
            "refresh_once" => RunMode::RefreshOnce,
            _ => RunMode::Api,
        };

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
            run_mode,
            database_url: env::var("DATABASE_URL")
                .ok()
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty()),
            enable_background_ingestion: env::var("TRUSTARANT_ENABLE_BACKGROUND_INGESTION")
                .ok()
                .map(|value| {
                    matches!(
                        value.to_ascii_lowercase().as_str(),
                        "1" | "true" | "yes" | "on"
                    )
                })
                .unwrap_or(false),
        }
    }
}
