resource "random_id" "project_suffix" {
  byte_length = 2
}

locals {
  project_id = lower("${var.project_id_prefix}-${random_id.project_suffix.hex}")

  required_apis = [
    "artifactregistry.googleapis.com",
    "cloudbuild.googleapis.com",
    "compute.googleapis.com",
    "iam.googleapis.com",
    "run.googleapis.com",
    "serviceusage.googleapis.com",
    "storage.googleapis.com"
  ]
}

resource "google_project" "cleanplated" {
  name                = var.project_name
  project_id          = local.project_id
  billing_account     = var.billing_account
  org_id              = var.org_id != "" ? var.org_id : null
  folder_id           = var.folder_id != "" ? var.folder_id : null
  auto_create_network = false
}

resource "google_project_service" "required" {
  for_each = toset(local.required_apis)

  project = google_project.cleanplated.project_id
  service = each.value

  disable_on_destroy = false
}

resource "google_storage_bucket" "terraform_state" {
  name          = "${google_project.cleanplated.project_id}-tfstate"
  location      = var.state_bucket_location
  project       = google_project.cleanplated.project_id
  force_destroy = false

  uniform_bucket_level_access = true
  public_access_prevention    = "enforced"

  versioning {
    enabled = true
  }

  lifecycle_rule {
    condition {
      age = 90
    }

    action {
      type = "Delete"
    }
  }

  depends_on = [google_project_service.required]
}
