# Trustarant Backend (Rust + Clean Architecture)

This backend is structured around clean architecture boundaries:

- `domain`: core entities and repository contracts
- `application`: directory/search and trust-score normalization use cases
- `infrastructure`: jurisdiction connectors, scheduler, and repository implementations
- `presentation`: HTTP transport (Axum)

## Run locally

```bash
cd backend
cp .env.example .env
cargo run
```

## Run with Docker Compose

From repository root:

```bash
docker compose up --build
```

## Core endpoints

- `GET /health`
- `GET /api/v1/facilities?q=sushi&latitude=34.0522&longitude=-118.2437&radius_miles=2&limit=20`
- `GET /api/v1/facilities/{id}`

## Notes

- Current ingestion connectors are scaffolded with realistic mock data for LA, San Diego, Long Beach, LIVES (Riverside/San Bernardino), and CPRA imports (Orange/Pasadena).
- Replace connector internals with real API/batch/CPRA data pipelines as you implement production ingestion.
