# Terraform Deployment (GCP)

This setup uses a two-step Terraform flow:

1. `bootstrap/` creates:
   - a new GCP project (name: `trustarant`)
   - required APIs
   - a remote state bucket
2. `environments/prod/` provisions runtime resources:
   - Cloud Run service for the Rust backend
   - Cloud Run service for the Vue frontend (nginx static container)
   - Artifact Registry repository
   - private static bucket (optional fallback)

## 1) Bootstrap project + state bucket

```bash
cd infra/terraform/bootstrap
cp terraform.tfvars.example terraform.tfvars
terraform init
terraform apply
terraform output
```

Capture outputs:
- `project_id`
- `terraform_state_bucket`

## 2) Initialize remote state + apply prod stack

```bash
cd ../environments/prod
cp terraform.tfvars.example terraform.tfvars
# Update terraform.tfvars with bootstrap project_id + backend image.
# Recommended (2026): set `disable_backend_invoker_iam_check = true` to make Cloud Run public
# without `allUsers` IAM binding. Or set explicit principals in `invoker_members`.
# If your org policy blocks bucket public access, keep `allow_public_frontend_bucket = false`.

terraform init \
  -backend-config="bucket=<terraform_state_bucket>" \
  -backend-config="prefix=terraform/prod"

terraform apply
```

## 3) Build and push frontend image

```bash
cd ../../../frontend
PROJECT_ID=<project_id>
REGION=us-west1

gcloud auth configure-docker ${REGION}-docker.pkg.dev
docker build -t ${REGION}-docker.pkg.dev/${PROJECT_ID}/trustarant/frontend:latest .
docker push ${REGION}-docker.pkg.dev/${PROJECT_ID}/trustarant/frontend:latest
```

## 4) Build and push backend image

```bash
cd ../../../backend
PROJECT_ID=<project_id>
REGION=us-west1

gcloud auth configure-docker ${REGION}-docker.pkg.dev

docker build -t ${REGION}-docker.pkg.dev/${PROJECT_ID}/trustarant/backend:latest .
docker push ${REGION}-docker.pkg.dev/${PROJECT_ID}/trustarant/backend:latest
```

Then rerun `terraform apply` in `environments/prod` with `backend_image` and `frontend_image` set.
