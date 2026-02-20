use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Jurisdiction {
    LosAngelesCounty,
    SanDiegoCounty,
    LongBeach,
    RiversideCounty,
    SanBernardinoCounty,
    OrangeCounty,
    Pasadena,
}

impl Jurisdiction {
    pub fn code(&self) -> &'static str {
        match self {
            Self::LosAngelesCounty => "lac",
            Self::SanDiegoCounty => "sdc",
            Self::LongBeach => "lb",
            Self::RiversideCounty => "riv",
            Self::SanBernardinoCounty => "sbc",
            Self::OrangeCounty => "oc",
            Self::Pasadena => "pas",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::LosAngelesCounty => "Los Angeles County",
            Self::SanDiegoCounty => "San Diego County",
            Self::LongBeach => "Long Beach",
            Self::RiversideCounty => "Riverside County",
            Self::SanBernardinoCounty => "San Bernardino County",
            Self::OrangeCounty => "Orange County",
            Self::Pasadena => "Pasadena",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Violation {
    pub code: String,
    pub description: String,
    pub points: i16,
    pub critical: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Inspection {
    pub inspection_id: String,
    pub inspected_at: DateTime<Utc>,
    pub raw_score: Option<f32>,
    pub letter_grade: Option<String>,
    pub placard_status: Option<String>,
    pub violations: Vec<Violation>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Facility {
    pub id: String,
    pub source_id: String,
    pub name: String,
    pub address: String,
    pub city: String,
    pub state: String,
    pub postal_code: String,
    pub latitude: f64,
    pub longitude: f64,
    pub jurisdiction: Jurisdiction,
    pub trust_score: u8,
    pub inspections: Vec<Inspection>,
    pub updated_at: DateTime<Utc>,
}
