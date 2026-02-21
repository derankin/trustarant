# Cleanplated Frontend (Vue 3 + IBM Carbon)

## Local development

```bash
cd frontend
cp .env.example .env
npm install
npm run dev
```

The app expects the backend at `VITE_API_BASE_URL` (default `http://localhost:8080`).

## UX Notes

- Mobile-first layout with IBM Carbon components/styles
- Browser geolocation mode for proximity search
- Ingestion/source transparency panel showing last refresh + per-connector record counts
- Client-side data slicing by jurisdiction, score band, and inspection recency

## Docker

Build and serve through the root compose stack:

```bash
docker compose up --build
```
