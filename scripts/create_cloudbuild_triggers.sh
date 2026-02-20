#!/usr/bin/env bash
set -euo pipefail

PROJECT_ID="${PROJECT_ID:-trustarant-2b14}"
REGION="${REGION:-us-west1}"
CONNECTION="${CONNECTION:-trustarant-conn}"
REPO_RESOURCE="projects/${PROJECT_ID}/locations/${REGION}/connections/${CONNECTION}/repositories/trustarant-repo"
BUILD_SA="projects/${PROJECT_ID}/serviceAccounts/cloudbuild-deployer@${PROJECT_ID}.iam.gserviceaccount.com"

# Requires connection installation_state COMPLETE.
gcloud builds repositories create trustarant-repo \
  --project="${PROJECT_ID}" \
  --region="${REGION}" \
  --connection="${CONNECTION}" \
  --remote-uri="https://github.com/derankin/trustarant.git" || true

gcloud builds triggers create github \
  --project="${PROJECT_ID}" \
  --region="${REGION}" \
  --name="trustarant-backend-main" \
  --repository="${REPO_RESOURCE}" \
  --branch-pattern='^main$' \
  --build-config='cloudbuild/backend.cloudbuild.yaml' \
  --service-account="${BUILD_SA}" || true

gcloud builds triggers create github \
  --project="${PROJECT_ID}" \
  --region="${REGION}" \
  --name="trustarant-frontend-main" \
  --repository="${REPO_RESOURCE}" \
  --branch-pattern='^main$' \
  --build-config='cloudbuild/frontend.cloudbuild.yaml' \
  --service-account="${BUILD_SA}" || true

gcloud builds triggers list --project="${PROJECT_ID}" --region="${REGION}"
