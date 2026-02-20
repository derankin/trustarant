use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::{application::dto::FacilitySearchQuery, presentation::http::AppState};

#[derive(Debug, Deserialize)]
pub struct FacilitySearchParams {
    pub q: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub radius_miles: Option<f64>,
    pub limit: Option<usize>,
}

#[derive(Debug, Serialize)]
pub struct HealthPayload {
    pub status: &'static str,
    pub timestamp: String,
}

pub async fn health() -> Json<HealthPayload> {
    Json(HealthPayload {
        status: "ok",
        timestamp: Utc::now().to_rfc3339(),
    })
}

pub async fn list_facilities(
    State(state): State<AppState>,
    Query(params): Query<FacilitySearchParams>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let facilities = state
        .directory_service
        .search(FacilitySearchQuery {
            q: params.q,
            latitude: params.latitude,
            longitude: params.longitude,
            radius_miles: params.radius_miles,
            limit: params.limit,
        })
        .await
        .map_err(internal_error)?;

    Ok(Json(
        serde_json::json!({ "count": facilities.len(), "data": facilities }),
    ))
}

pub async fn get_facility(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let facility = state
        .directory_service
        .get(&id)
        .await
        .map_err(internal_error)?;

    match facility {
        Some(record) => Ok(Json(serde_json::json!({ "data": record }))),
        None => Err((StatusCode::NOT_FOUND, "Facility not found".to_owned())),
    }
}

pub async fn ingestion_status(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let stats = state.ingestion_service.stats().await;

    Ok(Json(serde_json::json!({ "data": stats })))
}

fn internal_error(error: impl std::fmt::Display) -> (StatusCode, String) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        format!("internal error: {error}"),
    )
}
