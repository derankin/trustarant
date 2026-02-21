# Cleanplated Frontend (Vue 3 + IBM Carbon)

## Local development

```bash
cd frontend
cp .env.example .env
npm install
npm run dev
```

The app expects the backend at `VITE_API_BASE_URL` (default `http://localhost:8080`).

Google Analytics (GA4) is supported via runtime env var:
- `GA_MEASUREMENT_ID` (or `GOOGLE_ANALYTICS_ID`) e.g. `G-XXXXXXXXXX`

When set, the frontend server injects the `gtag.js` snippet into the rendered HTML (including share routes).

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

To run locally with GA enabled:

```bash
GA_MEASUREMENT_ID=G-XXXXXXXXXX docker compose up --build frontend
```
