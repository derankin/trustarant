use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::domain::entities::{Jurisdiction, Violation};

#[derive(Clone, Debug)]
pub struct SourceFacilityInput {
    pub source_id: String,
    pub name: String,
    pub address: String,
    pub city: String,
    pub state: String,
    pub postal_code: String,
    pub latitude: f64,
    pub longitude: f64,
    pub jurisdiction: Jurisdiction,
    pub inspected_at: DateTime<Utc>,
    pub raw_score: Option<f32>,
    pub letter_grade: Option<String>,
    pub placard_status: Option<String>,
    pub violations: Vec<Violation>,
}

#[derive(Clone, Debug)]
pub struct FacilitySearchQuery {
    pub q: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub radius_miles: Option<f64>,
    pub limit: Option<usize>,
}

#[derive(Clone, Debug, Serialize)]
pub struct FacilitySummary {
    pub id: String,
    pub name: String,
    pub address: String,
    pub city: String,
    pub state: String,
    pub postal_code: String,
    pub latitude: f64,
    pub longitude: f64,
    pub jurisdiction: String,
    pub trust_score: u8,
    pub latest_inspection_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, Serialize)]
pub struct FacilityDetail {
    pub id: String,
    pub source_id: String,
    pub name: String,
    pub address: String,
    pub city: String,
    pub state: String,
    pub postal_code: String,
    pub latitude: f64,
    pub longitude: f64,
    pub jurisdiction: String,
    pub trust_score: u8,
    pub inspections_count: usize,
    pub latest_inspection_at: Option<DateTime<Utc>>,
}
