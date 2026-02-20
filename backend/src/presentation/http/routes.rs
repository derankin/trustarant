use axum::{Router, routing::get};

use crate::presentation::http::{AppState, handlers};

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(handlers::health))
        .route("/api/v1/facilities", get(handlers::list_facilities))
        .route("/api/v1/facilities/{id}", get(handlers::get_facility))
        .route("/api/v1/system/ingestion", get(handlers::ingestion_status))
        .with_state(state)
}
