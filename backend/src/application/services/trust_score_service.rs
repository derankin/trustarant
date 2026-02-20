#[derive(Clone, Debug, Default)]
pub struct TrustScoreService;

#[derive(Clone, Debug, Default)]
pub struct ScoreSignals {
    pub raw_score: Option<f32>,
    pub letter_grade: Option<String>,
    pub placard_status: Option<String>,
}

impl TrustScoreService {
    pub fn score(&self, signals: &ScoreSignals) -> u8 {
        if let Some(raw) = signals.raw_score {
            return raw.clamp(0.0, 100.0) as u8;
        }

        if let Some(grade) = signals.letter_grade.as_deref() {
            return match grade.to_ascii_uppercase().as_str() {
                "A" => 95,
                "B" => 84,
                "C" => 74,
                _ => 65,
            };
        }

        if let Some(placard) = signals.placard_status.as_deref() {
            return match placard.to_ascii_lowercase().as_str() {
                "green" | "pass" => 95,
                "yellow" | "conditional" => 74,
                "red" | "closed" => 40,
                _ => 60,
            };
        }

        60
    }
}

#[cfg(test)]
mod tests {
    use super::{ScoreSignals, TrustScoreService};

    #[test]
    fn maps_numeric_scores_directly() {
        let service = TrustScoreService;
        let score = service.score(&ScoreSignals {
            raw_score: Some(91.2),
            ..ScoreSignals::default()
        });

        assert_eq!(score, 91);
    }

    #[test]
    fn maps_placard_scores_when_numeric_missing() {
        let service = TrustScoreService;
        let score = service.score(&ScoreSignals {
            placard_status: Some("yellow".to_owned()),
            ..ScoreSignals::default()
        });

        assert_eq!(score, 74);
    }
}
