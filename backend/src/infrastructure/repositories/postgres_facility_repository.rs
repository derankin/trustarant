use std::collections::HashMap;

use async_trait::async_trait;
use sqlx::{PgPool, Postgres, QueryBuilder, Row, postgres::PgPoolOptions};

use crate::{
    application::dto::{FacilitySearchQuery, ScoreSliceCounts},
    domain::{
        entities::{
            AutocompleteSuggestion, ConnectorIngestionStatus, Facility, FacilityVoteSummary,
            Inspection, Jurisdiction, SystemIngestionStatus, VoteValue,
        },
        errors::RepositoryError,
        repositories::FacilityRepository,
    },
};

pub struct PostgresFacilityRepository {
    pool: PgPool,
}

impl PostgresFacilityRepository {
    pub async fn connect(database_url: &str) -> Result<Self, RepositoryError> {
        let pool = PgPoolOptions::new()
            .max_connections(10)
            .connect(database_url)
            .await
            .map_err(to_repository_error)?;

        let repository = Self { pool };
        repository.init_schema().await?;

        Ok(repository)
    }

    async fn init_schema(&self) -> Result<(), RepositoryError> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS facilities (
                id TEXT PRIMARY KEY,
                source_id TEXT NOT NULL,
                name TEXT NOT NULL,
                address TEXT NOT NULL,
                city TEXT NOT NULL,
                state TEXT NOT NULL,
                postal_code TEXT NOT NULL,
                latitude DOUBLE PRECISION NOT NULL,
                longitude DOUBLE PRECISION NOT NULL,
                jurisdiction TEXT NOT NULL,
                trust_score SMALLINT NOT NULL,
                inspections JSONB NOT NULL,
                updated_at TIMESTAMPTZ NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(to_repository_error)?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_facilities_postal_code
            ON facilities (postal_code)
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(to_repository_error)?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS system_ingestion_status (
                id SMALLINT PRIMARY KEY CHECK (id = 1),
                last_refresh_at TIMESTAMPTZ NOT NULL,
                unique_facilities BIGINT NOT NULL,
                connector_stats JSONB NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(to_repository_error)?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS facility_votes (
                facility_id TEXT NOT NULL,
                voter_key TEXT NOT NULL,
                vote SMALLINT NOT NULL CHECK (vote IN (-1, 1)),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                PRIMARY KEY (facility_id, voter_key)
            )
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(to_repository_error)?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_facility_votes_facility_id
            ON facility_votes (facility_id)
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(to_repository_error)?;

        // Full-text search extensions and indexes
        sqlx::query("CREATE EXTENSION IF NOT EXISTS pg_trgm")
            .execute(&self.pool)
            .await
            .map_err(to_repository_error)?;

        sqlx::query("CREATE EXTENSION IF NOT EXISTS cube")
            .execute(&self.pool)
            .await
            .map_err(to_repository_error)?;

        sqlx::query("CREATE EXTENSION IF NOT EXISTS earthdistance")
            .execute(&self.pool)
            .await
            .map_err(to_repository_error)?;

        sqlx::query("ALTER TABLE facilities ADD COLUMN IF NOT EXISTS search_vector tsvector")
            .execute(&self.pool)
            .await
            .map_err(to_repository_error)?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_facilities_search_vector
            ON facilities USING GIN (search_vector)
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(to_repository_error)?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_facilities_name_trgm
            ON facilities USING GIN (name gin_trgm_ops)
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(to_repository_error)?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_facilities_city_trgm
            ON facilities USING GIN (city gin_trgm_ops)
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(to_repository_error)?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_facilities_geo
            ON facilities USING GIST (ll_to_earth(latitude, longitude))
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(to_repository_error)?;

        sqlx::query(
            r#"
            CREATE OR REPLACE FUNCTION facilities_search_vector_update() RETURNS trigger AS $$
            BEGIN
              NEW.search_vector :=
                setweight(to_tsvector('english', COALESCE(NEW.name, '')), 'A') ||
                setweight(to_tsvector('english', COALESCE(NEW.address, '')), 'B') ||
                setweight(to_tsvector('english', COALESCE(NEW.city, '')), 'B') ||
                setweight(to_tsvector('simple', COALESCE(NEW.postal_code, '')), 'C');
              RETURN NEW;
            END;
            $$ LANGUAGE plpgsql
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(to_repository_error)?;

        sqlx::query("DROP TRIGGER IF EXISTS trg_facilities_search_vector ON facilities")
            .execute(&self.pool)
            .await
            .map_err(to_repository_error)?;

        sqlx::query(
            r#"
            CREATE TRIGGER trg_facilities_search_vector
              BEFORE INSERT OR UPDATE ON facilities
              FOR EACH ROW EXECUTE FUNCTION facilities_search_vector_update()
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(to_repository_error)?;

        // Backfill existing rows that have no search_vector yet
        sqlx::query(
            r#"
            UPDATE facilities SET search_vector =
              setweight(to_tsvector('english', COALESCE(name, '')), 'A') ||
              setweight(to_tsvector('english', COALESCE(address, '')), 'B') ||
              setweight(to_tsvector('english', COALESCE(city, '')), 'B') ||
              setweight(to_tsvector('simple', COALESCE(postal_code, '')), 'C')
            WHERE search_vector IS NULL
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(to_repository_error)?;

        Ok(())
    }
}

#[async_trait]
impl FacilityRepository for PostgresFacilityRepository {
    async fn replace_all(&self, facilities: Vec<Facility>) -> Result<(), RepositoryError> {
        let mut transaction = self.pool.begin().await.map_err(to_repository_error)?;

        sqlx::query("TRUNCATE TABLE facilities")
            .execute(&mut *transaction)
            .await
            .map_err(to_repository_error)?;

        if !facilities.is_empty() {
            for chunk in facilities.chunks(1_000) {
                let mut builder = QueryBuilder::<Postgres>::new(
                    "INSERT INTO facilities (id, source_id, name, address, city, state, postal_code, latitude, longitude, jurisdiction, trust_score, inspections, updated_at) ",
                );

                builder.push_values(chunk.iter(), |mut row, facility| {
                    let inspections = serde_json::to_value(&facility.inspections)
                        .unwrap_or_else(|_| serde_json::json!([]));

                    row.push_bind(&facility.id)
                        .push_bind(&facility.source_id)
                        .push_bind(&facility.name)
                        .push_bind(&facility.address)
                        .push_bind(&facility.city)
                        .push_bind(&facility.state)
                        .push_bind(&facility.postal_code)
                        .push_bind(facility.latitude)
                        .push_bind(facility.longitude)
                        .push_bind(facility.jurisdiction.code())
                        .push_bind(i16::from(facility.trust_score))
                        .push_bind(inspections)
                        .push_bind(facility.updated_at);
                });

                builder
                    .build()
                    .execute(&mut *transaction)
                    .await
                    .map_err(to_repository_error)?;
            }
        }

        transaction.commit().await.map_err(to_repository_error)?;
        Ok(())
    }

    async fn list(&self) -> Result<Vec<Facility>, RepositoryError> {
        let rows = sqlx::query(
            "SELECT id, source_id, name, address, city, state, postal_code, latitude, longitude, jurisdiction, trust_score, inspections, updated_at FROM facilities",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(to_repository_error)?;

        rows.into_iter().map(map_facility_row).collect()
    }

    async fn get_by_id(&self, id: &str) -> Result<Option<Facility>, RepositoryError> {
        let maybe_row = sqlx::query(
            "SELECT id, source_id, name, address, city, state, postal_code, latitude, longitude, jurisdiction, trust_score, inspections, updated_at FROM facilities WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(to_repository_error)?;

        maybe_row.map(map_facility_row).transpose()
    }

    async fn set_system_ingestion_status(
        &self,
        status: SystemIngestionStatus,
    ) -> Result<(), RepositoryError> {
        let connector_stats = serde_json::to_value(status.connector_stats)
            .map_err(|error| RepositoryError::message(error.to_string()))?;

        sqlx::query(
            r#"
            INSERT INTO system_ingestion_status (id, last_refresh_at, unique_facilities, connector_stats)
            VALUES (1, $1, $2, $3)
            ON CONFLICT (id)
            DO UPDATE SET
                last_refresh_at = EXCLUDED.last_refresh_at,
                unique_facilities = EXCLUDED.unique_facilities,
                connector_stats = EXCLUDED.connector_stats
            "#,
        )
        .bind(status.last_refresh_at)
        .bind(status.unique_facilities as i64)
        .bind(connector_stats)
        .execute(&self.pool)
        .await
        .map_err(to_repository_error)?;

        Ok(())
    }

    async fn get_system_ingestion_status(
        &self,
    ) -> Result<Option<SystemIngestionStatus>, RepositoryError> {
        let maybe_row = sqlx::query(
            "SELECT last_refresh_at, unique_facilities, connector_stats FROM system_ingestion_status WHERE id = 1",
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(to_repository_error)?;

        maybe_row
            .map(|row| {
                let connector_stats_json: serde_json::Value = row.get("connector_stats");
                let connector_stats: Vec<ConnectorIngestionStatus> =
                    serde_json::from_value(connector_stats_json).map_err(|error| {
                        RepositoryError::message(format!(
                            "unable to decode connector stats: {error}"
                        ))
                    })?;

                let unique_facilities_raw: i64 = row.get("unique_facilities");
                Ok(SystemIngestionStatus {
                    last_refresh_at: row.get("last_refresh_at"),
                    unique_facilities: usize::try_from(unique_facilities_raw).unwrap_or(0),
                    connector_stats,
                })
            })
            .transpose()
    }

    async fn upsert_facility_vote(
        &self,
        facility_id: &str,
        voter_key: &str,
        vote: VoteValue,
    ) -> Result<FacilityVoteSummary, RepositoryError> {
        sqlx::query(
            r#"
            INSERT INTO facility_votes (facility_id, voter_key, vote, updated_at)
            VALUES ($1, $2, $3, NOW())
            ON CONFLICT (facility_id, voter_key)
            DO UPDATE SET
                vote = EXCLUDED.vote,
                updated_at = EXCLUDED.updated_at
            "#,
        )
        .bind(facility_id)
        .bind(voter_key)
        .bind(vote.to_i16())
        .execute(&self.pool)
        .await
        .map_err(to_repository_error)?;

        let summaries = self
            .get_facility_vote_summaries(&[facility_id.to_owned()])
            .await?;
        Ok(summaries.get(facility_id).cloned().unwrap_or_default())
    }

    async fn get_facility_vote_summaries(
        &self,
        facility_ids: &[String],
    ) -> Result<HashMap<String, FacilityVoteSummary>, RepositoryError> {
        if facility_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let rows = sqlx::query(
            r#"
            SELECT
                facility_id,
                COALESCE(SUM(CASE WHEN vote = 1 THEN 1 ELSE 0 END), 0) AS likes,
                COALESCE(SUM(CASE WHEN vote = -1 THEN 1 ELSE 0 END), 0) AS dislikes
            FROM facility_votes
            WHERE facility_id = ANY($1)
            GROUP BY facility_id
            "#,
        )
        .bind(facility_ids)
        .fetch_all(&self.pool)
        .await
        .map_err(to_repository_error)?;

        let mut summaries = HashMap::new();
        for row in rows {
            let facility_id: String = row.get("facility_id");
            let likes: i64 = row.get("likes");
            let dislikes: i64 = row.get("dislikes");
            summaries.insert(
                facility_id,
                FacilityVoteSummary {
                    likes: likes.max(0) as u64,
                    dislikes: dislikes.max(0) as u64,
                },
            );
        }

        Ok(summaries)
    }

    async fn search_facilities(
        &self,
        query: &FacilitySearchQuery,
    ) -> Result<(Vec<Facility>, usize, ScoreSliceCounts), RepositoryError> {
        let has_search_term = query
            .q
            .as_ref()
            .map(|v| !v.trim().is_empty())
            .unwrap_or(false);

        let page_size = query.page_size.or(query.limit).unwrap_or(50).clamp(1, 200);
        let page = query.page.unwrap_or(1).max(1);
        let offset = (page - 1).saturating_mul(page_size);

        let mut builder = QueryBuilder::<Postgres>::new("");

        if has_search_term {
            let term = query.q.as_ref().unwrap().trim();
            // Text search mode: FTS + trigram fuzzy matching
            builder.push(
                r#"
                WITH scored AS (
                    SELECT f.*,
                        ts_rank(f.search_vector, plainto_tsquery('english', "#,
            );
            builder.push_bind(term);
            builder.push(
                r#")) AS fts_rank,
                        similarity(f.name, "#,
            );
            builder.push_bind(term);
            builder.push(") AS name_sim");

            // If geo coords are provided, add distance as ranking signal
            if let (Some(lat), Some(lon)) = (query.latitude, query.longitude) {
                builder.push(
                    r#",
                        earth_distance(ll_to_earth(f.latitude, f.longitude), ll_to_earth("#,
                );
                builder.push_bind(lat);
                builder.push(", ");
                builder.push_bind(lon);
                builder.push(")) AS dist_meters");
            }

            builder.push(
                r#"
                    FROM facilities f
                    WHERE (
                        f.search_vector @@ plainto_tsquery('english', "#,
            );
            builder.push_bind(term);
            builder.push(
                r#")
                        OR similarity(f.name, "#,
            );
            builder.push_bind(term);
            builder.push(") > 0.15)");
        } else {
            // Geo browse mode
            builder.push(
                r#"
                WITH scored AS (
                    SELECT f.*, 0::real AS fts_rank, 0::real AS name_sim"#,
            );

            if let (Some(lat), Some(lon)) = (query.latitude, query.longitude) {
                builder.push(
                    r#",
                        earth_distance(ll_to_earth(f.latitude, f.longitude), ll_to_earth("#,
                );
                builder.push_bind(lat);
                builder.push(", ");
                builder.push_bind(lon);
                builder.push(")) AS dist_meters");
            }

            builder.push(
                r#"
                    FROM facilities f
                    WHERE 1=1"#,
            );

            if let (Some(lat), Some(lon), Some(radius)) =
                (query.latitude, query.longitude, query.radius_miles)
            {
                let radius_meters = radius.max(0.1) * 1609.344;
                builder.push(
                    r#"
                        AND earth_distance(ll_to_earth(f.latitude, f.longitude), ll_to_earth("#,
                );
                builder.push_bind(lat);
                builder.push(", ");
                builder.push_bind(lon);
                builder.push(")) <= ");
                builder.push_bind(radius_meters);
            }
        }

        // Jurisdiction filter
        if let Some(jurisdiction) = query
            .jurisdiction
            .as_ref()
            .map(|v| v.trim().to_ascii_lowercase())
        {
            if !jurisdiction.is_empty() && jurisdiction != "all" {
                // Support both jurisdiction codes (e.g. "lac") and labels (e.g. "Los Angeles County")
                builder.push(" AND (LOWER(f.jurisdiction) = ");
                builder.push_bind(jurisdiction.clone());
                builder.push(" OR LOWER(f.jurisdiction) = ");
                // Try to map label to code for backward compatibility
                let code = match jurisdiction.as_str() {
                    "los angeles county" => "lac",
                    "san diego county" => "sdc",
                    "long beach" => "lb",
                    "riverside county" => "riv",
                    "san bernardino county" => "sbc",
                    "orange county" => "oc",
                    "pasadena" => "pas",
                    other => other,
                };
                builder.push_bind(code.to_owned());
                builder.push(")");
            }
        }

        // Recent-only filter
        if query.recent_only.unwrap_or(false) {
            builder.push(" AND f.updated_at >= NOW() - INTERVAL '90 days'");
        }

        builder.push(
            r#"
            )
            SELECT s.*,
                COUNT(*) OVER() AS total_count,
                COUNT(*) FILTER(WHERE s.trust_score >= 90) OVER() AS elite_count,
                COUNT(*) FILTER(WHERE s.trust_score >= 80 AND s.trust_score < 90) OVER() AS solid_count,
                COUNT(*) FILTER(WHERE s.trust_score < 80) OVER() AS watch_count
            FROM scored s
            WHERE 1=1"#,
        );

        // Score slice filter (applied after CTE so slice counts reflect pre-filter totals)
        if let Some(slice) = query
            .score_slice
            .as_ref()
            .map(|v| v.trim().to_ascii_lowercase())
        {
            match slice.as_str() {
                "elite" => builder.push(" AND s.trust_score >= 90"),
                "solid" => builder.push(" AND s.trust_score >= 80 AND s.trust_score < 90"),
                "watch" => builder.push(" AND s.trust_score < 80"),
                _ => {}
            };
        }

        // Sorting
        let sort = query
            .sort
            .as_ref()
            .map(|v| v.trim().to_ascii_lowercase());
        match sort.as_deref() {
            Some("recent_desc") => {
                builder.push(" ORDER BY s.updated_at DESC, s.trust_score DESC");
            }
            Some("name_asc") => {
                builder.push(" ORDER BY s.name ASC");
            }
            _ => {
                if has_search_term {
                    builder.push(
                        " ORDER BY (s.fts_rank * 10 + s.name_sim * 5) DESC, s.trust_score DESC",
                    );
                } else {
                    builder.push(" ORDER BY s.trust_score DESC, s.updated_at DESC");
                }
            }
        }

        builder.push(" LIMIT ");
        builder.push_bind(page_size as i64);
        builder.push(" OFFSET ");
        builder.push_bind(offset as i64);

        let rows = builder
            .build()
            .fetch_all(&self.pool)
            .await
            .map_err(to_repository_error)?;

        let mut total_count: usize = 0;
        let mut elite_count: usize = 0;
        let mut solid_count: usize = 0;
        let mut watch_count: usize = 0;
        let mut facilities = Vec::with_capacity(rows.len());

        for row in &rows {
            if total_count == 0 {
                let tc: i64 = row.get("total_count");
                let ec: i64 = row.get("elite_count");
                let sc: i64 = row.get("solid_count");
                let wc: i64 = row.get("watch_count");
                total_count = tc.max(0) as usize;
                elite_count = ec.max(0) as usize;
                solid_count = sc.max(0) as usize;
                watch_count = wc.max(0) as usize;
            }
            facilities.push(map_facility_row_ref(row)?);
        }

        let slice_counts = ScoreSliceCounts {
            all: total_count,
            elite: elite_count,
            solid: solid_count,
            watch: watch_count,
        };

        Ok((facilities, total_count, slice_counts))
    }

    async fn autocomplete(
        &self,
        prefix: &str,
        limit: usize,
    ) -> Result<Vec<AutocompleteSuggestion>, RepositoryError> {
        let like_pattern = format!("%{}%", prefix.replace('%', "\\%").replace('_', "\\_"));
        let prefix_pattern = format!("{}%", prefix.replace('%', "\\%").replace('_', "\\_"));
        let capped_limit = limit.clamp(1, 20) as i64;

        let rows = sqlx::query(
            r#"
            SELECT id, name, city, postal_code, trust_score,
                   similarity(name, $1) AS sim
            FROM facilities
            WHERE name ILIKE $2
               OR city ILIKE $2
               OR postal_code LIKE $3
            ORDER BY sim DESC, trust_score DESC
            LIMIT $4
            "#,
        )
        .bind(prefix)
        .bind(&like_pattern)
        .bind(&prefix_pattern)
        .bind(capped_limit)
        .fetch_all(&self.pool)
        .await
        .map_err(to_repository_error)?;

        let suggestions = rows
            .iter()
            .map(|row| {
                let trust_score_raw: i16 = row.get("trust_score");
                AutocompleteSuggestion {
                    id: row.get("id"),
                    name: row.get("name"),
                    city: row.get("city"),
                    postal_code: row.get("postal_code"),
                    trust_score: u8::try_from(trust_score_raw).unwrap_or(0),
                }
            })
            .collect();

        Ok(suggestions)
    }
}

fn map_facility_row_ref(row: &sqlx::postgres::PgRow) -> Result<Facility, RepositoryError> {
    let jurisdiction_code: String = row.get("jurisdiction");
    let inspections_json: serde_json::Value = row.get("inspections");
    let trust_score_raw: i16 = row.get("trust_score");

    let jurisdiction = Jurisdiction::from_code(&jurisdiction_code).ok_or_else(|| {
        RepositoryError::message(format!("unknown jurisdiction code: {jurisdiction_code}"))
    })?;

    let inspections: Vec<Inspection> =
        serde_json::from_value(inspections_json).map_err(|error| {
            RepositoryError::message(format!("unable to decode inspections: {error}"))
        })?;

    Ok(Facility {
        id: row.get("id"),
        source_id: row.get("source_id"),
        name: row.get("name"),
        address: row.get("address"),
        city: row.get("city"),
        state: row.get("state"),
        postal_code: row.get("postal_code"),
        latitude: row.get("latitude"),
        longitude: row.get("longitude"),
        jurisdiction,
        trust_score: u8::try_from(trust_score_raw).unwrap_or(0),
        inspections,
        updated_at: row.get("updated_at"),
    })
}

fn map_facility_row(row: sqlx::postgres::PgRow) -> Result<Facility, RepositoryError> {
    let jurisdiction_code: String = row.get("jurisdiction");
    let inspections_json: serde_json::Value = row.get("inspections");
    let trust_score_raw: i16 = row.get("trust_score");

    let jurisdiction = Jurisdiction::from_code(&jurisdiction_code).ok_or_else(|| {
        RepositoryError::message(format!("unknown jurisdiction code: {jurisdiction_code}"))
    })?;

    let inspections: Vec<Inspection> =
        serde_json::from_value(inspections_json).map_err(|error| {
            RepositoryError::message(format!("unable to decode inspections: {error}"))
        })?;

    Ok(Facility {
        id: row.get("id"),
        source_id: row.get("source_id"),
        name: row.get("name"),
        address: row.get("address"),
        city: row.get("city"),
        state: row.get("state"),
        postal_code: row.get("postal_code"),
        latitude: row.get("latitude"),
        longitude: row.get("longitude"),
        jurisdiction,
        trust_score: u8::try_from(trust_score_raw).unwrap_or(0),
        inspections,
        updated_at: row.get("updated_at"),
    })
}

fn to_repository_error(error: impl std::fmt::Display) -> RepositoryError {
    RepositoryError::message(error.to_string())
}
