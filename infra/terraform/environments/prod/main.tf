locals {
  required_apis = [
    "artifactregistry.googleapis.com",
    "cloudbuild.googleapis.com",
    "iam.googleapis.com",
    "run.googleapis.com",
    "storage.googleapis.com"
  ]

  frontend_bucket_name = var.frontend_bucket_name != "" ? var.frontend_bucket_name : "${var.project_id}-trustarant-web"
}

resource "google_project_service" "required" {
  for_each = toset(local.required_apis)

  project = var.project_id
  service = each.value

  disable_on_destroy = false
}

resource "google_artifact_registry_repository" "backend" {
  project       = var.project_id
  location      = var.region
  repository_id = "trustarant"
  description   = "Container registry for Trustarant services"
  format        = "DOCKER"

  depends_on = [google_project_service.required]
}

resource "google_service_account" "cloud_run_runtime" {
  project      = var.project_id
  account_id   = "trustarant-api"
  display_name = "Trustarant Cloud Run runtime"

  depends_on = [google_project_service.required]
}

resource "google_cloud_run_v2_service" "api" {
  project              = var.project_id
  name                 = "trustarant-api"
  location             = var.region
  ingress              = "INGRESS_TRAFFIC_ALL"
  invoker_iam_disabled = var.disable_backend_invoker_iam_check

  template {
    service_account = google_service_account.cloud_run_runtime.email

    scaling {
      min_instance_count = 0
      max_instance_count = 3
    }

    containers {
      image = var.backend_image

      ports {
        container_port = 8080
      }

      env {
        name  = "TRUSTARANT_PORT"
        value = "8080"
      }

      env {
        name  = "TRUSTARANT_HOST"
        value = "0.0.0.0"
      }
    }
  }

  depends_on = [google_project_service.required]
}

resource "google_cloud_run_v2_service_iam_member" "public_invoker" {
  for_each = toset(var.invoker_members)

  project  = var.project_id
  location = var.region
  name     = google_cloud_run_v2_service.api.name
  role     = "roles/run.invoker"
  member   = each.value
}

resource "google_storage_bucket" "frontend" {
  project       = var.project_id
  name          = local.frontend_bucket_name
  location      = "US"
  force_destroy = true

  uniform_bucket_level_access = true
  public_access_prevention    = "inherited"

  website {
    main_page_suffix = "index.html"
    not_found_page   = "index.html"
  }

  depends_on = [google_project_service.required]
}

resource "google_storage_bucket_iam_member" "frontend_public_read" {
  count = var.allow_public_frontend_bucket ? 1 : 0

  bucket = google_storage_bucket.frontend.name
  role   = "roles/storage.objectViewer"
  member = "allUsers"
}
