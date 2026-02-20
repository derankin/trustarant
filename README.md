# Trustarant Monorepo

Trustarant centralizes Southern California restaurant health inspection data and exposes normalized Trust Scores through a mobile-first directory.

## Structure

- `backend/`: Rust backend (clean architecture + ingestion scheduler)
- `frontend/`: Vue 3 + Tailwind v4 mobile-first client
- `infra/terraform/`: GCP bootstrap + deployment infrastructure
- `cloudbuild/`: CI/CD configs for backend and frontend on `main`
- `docs/research/socal-food-safety-data-strategy.md`: research blueprint and source references

## Quick start

```bash
# Full stack with Docker Compose
docker compose up --build
```

Services:

- Frontend: `http://localhost:15173`
- Backend API: `http://localhost:18080`
- Postgres: internal compose service (`postgres:5432`)
- Redis: internal compose service (`redis:6379`)
