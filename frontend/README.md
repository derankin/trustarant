# Trustarant Frontend (Vue 3 + Tailwind v4)

## Local development

```bash
cd frontend
cp .env.example .env
npm install
npm run dev
```

The app expects the backend at `VITE_API_BASE_URL` (default `http://localhost:8080`).

## Docker

Build and serve through the root compose stack:

```bash
docker compose up --build
```
