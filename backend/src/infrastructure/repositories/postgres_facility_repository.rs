use std::collections::HashMap;

use async_trait::async_trait;
use sqlx::{PgPool, Postgres, QueryBuilder, Row, postgres::PgPoolOptions};

use crate::domain::{
    entities::{
        ConnectorIngestionStatus, Facility, FacilityVoteSummary, Inspection, Jurisdiction,
        SystemIngestionStatus, VoteValue,
    },
    errors::RepositoryError,
    repositories::FacilityRepository,
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
