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

## Live Data Connectors

### San Diego (Socrata)

`SanDiegoConnector` pulls from the live SODA dataset:

- Base URL: `https://internal-sandiegocounty.data.socrata.com`
- Dataset ID: `c5ez-ufrd` (`Food Facility Permits`)

Configure with environment variables:

- `TRUSTARANT_SD_SOCRATA_BASE_URL`
- `TRUSTARANT_SD_SOCRATA_DATASET_ID`
- `TRUSTARANT_SD_SOCRATA_LIMIT`
- `TRUSTARANT_SD_SOCRATA_ACTIVE_ONLY`
- `TRUSTARANT_SD_SOCRATA_TIMEOUT_SECS`
- `TRUSTARANT_SD_SOCRATA_APP_TOKEN` (optional but recommended)

The implementation references:

- `docs/research/socal-food-safety-data-strategy.md`
- `docs/research/strategic-framework-full.md`

### Los Angeles County (ArcGIS)

`LaCountyConnector` calls live ArcGIS FeatureServer feeds for:

- Facility inventory
- Inspections
- Violations

Config:

- `TRUSTARANT_LA_INVENTORY_URL`
- `TRUSTARANT_LA_INSPECTIONS_URL`
- `TRUSTARANT_LA_VIOLATIONS_URL`
- `TRUSTARANT_LA_LIMIT`
- `TRUSTARANT_LA_TIMEOUT_SECS`

### Long Beach (Live web page)

`LongBeachConnector` fetches the live Long Beach restaurant-closures page.

Config:

- `TRUSTARANT_LONG_BEACH_CLOSURES_URL`
- `TRUSTARANT_LONG_BEACH_LIMIT`
- `TRUSTARANT_LONG_BEACH_TIMEOUT_SECS`

### LIVES Batch (San Bernardino + Riverside optional)

`LivesBatchConnector` fetches live ArcGIS records:

- San Bernardino: default live FeatureServer endpoint
- Riverside: optional live FeatureServer URL via env (set when available)

Config:

- `TRUSTARANT_SBC_ARCGIS_URL`
- `TRUSTARANT_RIVERSIDE_ARCGIS_URL` (optional)
- `TRUSTARANT_LIVES_LIMIT`
- `TRUSTARANT_LIVES_TIMEOUT_SECS`

### CPRA Imports (Orange + Pasadena)

`CpraConnector` fetches real CPRA export URLs (CSV or JSON) when provided.

Config:

- `TRUSTARANT_OC_CPRA_EXPORT_URL`
- `TRUSTARANT_PASADENA_CPRA_EXPORT_URL`
- `TRUSTARANT_CPRA_TIMEOUT_SECS`

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

- All connectors now use live network calls and emit no hardcoded sample facilities.
- Riverside and CPRA sources are environment-driven where direct public machine-readable feeds are not consistently available.
- The San Diego feed currently exposes permit-status metadata; Trust Score signals are derived from those fields until full inspection-line datasets are integrated.
