mod application;
mod config;
mod domain;
mod infrastructure;
mod presentation;

use std::{net::SocketAddr, sync::Arc};

use application::services::{DirectoryService, IngestionService, TrustScoreService};
use axum::Router;
use config::Settings;
use infrastructure::{
    connectors::default_connectors, repositories::InMemoryFacilityRepository, scheduler,
};
use presentation::http::{routes::build_router, AppState};
use tokio::net::TcpListener;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::{error, info};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();

    let settings = Settings::from_env();

    let repository = Arc::new(InMemoryFacilityRepository::new());
    let trust_score_service = Arc::new(TrustScoreService::default());
    let ingestion_service = Arc::new(IngestionService::new(
        repository.clone(),
        trust_score_service,
        default_connectors(),
    ));

    if let Err(err) = ingestion_service.refresh().await {
        error!(error = %err, "Initial ingestion failed; API will still start");
    }

    let app_state = AppState {
        directory_service: Arc::new(DirectoryService::new(repository)),
    };

    tokio::spawn(scheduler::run(
        ingestion_service,
        settings.ingestion_interval_hours,
    ));

    let app = app_router(app_state, &settings);
    let addr: SocketAddr = format!("{}:{}", settings.host, settings.port).parse()?;
    let listener = TcpListener::bind(addr).await?;

    info!(address = %addr, "Trustarant backend listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

fn app_router(state: AppState, settings: &Settings) -> Router {
    let allow_origin = settings
        .cors_origin
        .parse::<axum::http::HeaderValue>()
        .unwrap_or_else(|_| axum::http::HeaderValue::from_static("http://localhost:5173"));

    let cors = CorsLayer::new()
        .allow_origin(allow_origin)
        .allow_methods(tower_http::cors::Any)
        .allow_headers(tower_http::cors::Any);

    build_router(state).layer(TraceLayer::new_for_http()).layer(cors)
}

fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "trustarant_backend=info,tower_http=info".into()),
        )
        .init();
}

async fn shutdown_signal() {
    if let Err(error) = tokio::signal::ctrl_c().await {
        error!(%error, "Unable to listen for shutdown signal");
    }

    info!("Shutdown signal received");
}
