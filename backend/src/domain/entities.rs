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

    pub fn from_code(code: &str) -> Option<Self> {
        match code {
            "lac" => Some(Self::LosAngelesCounty),
            "sdc" => Some(Self::SanDiegoCounty),
            "lb" => Some(Self::LongBeach),
            "riv" => Some(Self::RiversideCounty),
            "sbc" => Some(Self::SanBernardinoCounty),
            "oc" => Some(Self::OrangeCounty),
            "pas" => Some(Self::Pasadena),
            _ => None,
        }
    }

    pub fn from_label(label: &str) -> Option<Self> {
        match label.to_ascii_lowercase().as_str() {
            "los angeles county" => Some(Self::LosAngelesCounty),
            "san diego county" => Some(Self::SanDiegoCounty),
            "long beach" => Some(Self::LongBeach),
            "riverside county" => Some(Self::RiversideCounty),
            "san bernardino county" => Some(Self::SanBernardinoCounty),
            "orange county" => Some(Self::OrangeCounty),
            "pasadena" => Some(Self::Pasadena),
            _ => None,
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

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ConnectorIngestionStatus {
    pub source: String,
    pub fetched_records: usize,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SystemIngestionStatus {
    pub last_refresh_at: DateTime<Utc>,
    pub unique_facilities: usize,
    pub connector_stats: Vec<ConnectorIngestionStatus>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum VoteValue {
    Like,
    Dislike,
}

impl VoteValue {
    pub fn to_i16(&self) -> i16 {
        match self {
            Self::Like => 1,
            Self::Dislike => -1,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct FacilityVoteSummary {
    pub likes: u64,
    pub dislikes: u64,
}

impl FacilityVoteSummary {
    pub fn score(&self) -> i64 {
        self.likes as i64 - self.dislikes as i64
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AutocompleteSuggestion {
    pub id: String,
    pub name: String,
    pub city: String,
    pub postal_code: String,
    pub trust_score: u8,
}

#[derive(Clone, Debug)]
pub struct FacilitySearchQuery {
    pub q: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub radius_miles: Option<f64>,
    pub jurisdiction: Option<String>,
    pub sort: Option<String>,
    pub score_slice: Option<String>,
    pub recent_only: Option<bool>,
    pub page: Option<usize>,
    pub page_size: Option<usize>,
    pub limit: Option<usize>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ScoreSliceCounts {
    pub all: usize,
    pub elite: usize,
    pub solid: usize,
    pub watch: usize,
}
