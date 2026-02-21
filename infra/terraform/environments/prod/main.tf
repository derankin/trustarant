locals {
  required_apis = [
    "artifactregistry.googleapis.com",
    "cloudbuild.googleapis.com",
    "cloudscheduler.googleapis.com",
    "iam.googleapis.com",
    "run.googleapis.com",
    "secretmanager.googleapis.com",
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

resource "google_service_account" "scheduler_invoker" {
  project      = var.project_id
  account_id   = "trustarant-scheduler"
  display_name = "Trustarant Scheduler Invoker"

  depends_on = [google_project_service.required]
}

resource "google_project_iam_member" "scheduler_run_developer" {
  project = var.project_id
  role    = "roles/run.developer"
  member  = "serviceAccount:${google_service_account.scheduler_invoker.email}"
}

resource "google_service_account_iam_member" "scheduler_service_account_user" {
  service_account_id = google_service_account.cloud_run_runtime.name
  role               = "roles/iam.serviceAccountUser"
  member             = "serviceAccount:${google_service_account.scheduler_invoker.email}"
}

resource "google_project_iam_member" "runtime_secret_accessor" {
  project = var.project_id
  role    = "roles/secretmanager.secretAccessor"
  member  = "serviceAccount:${google_service_account.cloud_run_runtime.email}"
}

resource "google_secret_manager_secret" "database_url" {
  project   = var.project_id
  secret_id = var.database_url_secret_id

  replication {
    auto {}
  }

  depends_on = [google_project_service.required]
}

resource "google_secret_manager_secret_iam_member" "runtime_secret_accessor_on_database_url" {
  project   = var.project_id
  secret_id = google_secret_manager_secret.database_url.secret_id
  role      = "roles/secretmanager.secretAccessor"
  member    = "serviceAccount:${google_service_account.cloud_run_runtime.email}"
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

      env {
        name  = "TRUSTARANT_CORS_ORIGIN"
        value = "*"
      }

      env {
        name  = "TRUSTARANT_RUN_MODE"
        value = "api"
      }

      env {
        name  = "TRUSTARANT_ENABLE_BACKGROUND_INGESTION"
        value = "false"
      }

      env {
        name = "DATABASE_URL"
        value_source {
          secret_key_ref {
            secret  = google_secret_manager_secret.database_url.secret_id
            version = "latest"
          }
        }
      }
    }
  }

  depends_on = [
    google_project_service.required,
    google_secret_manager_secret_iam_member.runtime_secret_accessor_on_database_url,
  ]
}

resource "google_cloud_run_v2_job" "ingestion" {
  project  = var.project_id
  name     = "trustarant-ingestion"
  location = var.region
  deletion_protection = false

  template {
    template {
      service_account = google_service_account.cloud_run_runtime.email
      timeout         = "3600s"
      max_retries     = 3

      containers {
        image = var.backend_image

        env {
          name  = "TRUSTARANT_RUN_MODE"
          value = "refresh_once"
        }

        env {
          name = "DATABASE_URL"
          value_source {
            secret_key_ref {
              secret  = google_secret_manager_secret.database_url.secret_id
              version = "latest"
            }
          }
        }
      }
    }
  }

  depends_on = [
    google_project_service.required,
    google_secret_manager_secret_iam_member.runtime_secret_accessor_on_database_url,
  ]
}

resource "google_cloud_run_v2_service" "frontend" {
  project              = var.project_id
  name                 = "trustarant-web"
  location             = var.region
  ingress              = "INGRESS_TRAFFIC_ALL"
  invoker_iam_disabled = var.disable_frontend_invoker_iam_check

  template {
    service_account = google_service_account.cloud_run_runtime.email

    scaling {
      min_instance_count = 0
      max_instance_count = 3
    }

    containers {
      image = var.frontend_image

      ports {
        container_port = 80
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

resource "google_cloud_scheduler_job" "ingestion_refresh" {
  count = var.enable_ingestion_scheduler ? 1 : 0

  project     = var.project_id
  name        = "trustarant-ingestion-refresh"
  description = "Triggers backend ingestion refresh"
  region      = var.region
  schedule    = var.ingestion_refresh_schedule
  time_zone   = "Etc/UTC"

  http_target {
    http_method = "POST"
    uri         = "https://run.googleapis.com/v2/projects/${var.project_id}/locations/${var.region}/jobs/${google_cloud_run_v2_job.ingestion.name}:run"

    headers = {
      "Content-Type" = "application/json"
    }

    body = base64encode("{}")

    oauth_token {
      service_account_email = google_service_account.scheduler_invoker.email
      scope                 = "https://www.googleapis.com/auth/cloud-platform"
    }
  }

  retry_config {
    retry_count          = 3
    min_backoff_duration = "30s"
    max_backoff_duration = "600s"
    max_retry_duration   = "3600s"
  }

  depends_on = [google_project_service.required]
}
