output "project_id" {
  description = "Newly created project ID"
  value       = google_project.cleanplated.project_id
}

output "project_name" {
  description = "Newly created project display name"
  value       = google_project.cleanplated.name
}

output "region" {
  description = "Default region to use in downstream stacks"
  value       = var.region
}

output "terraform_state_bucket" {
  description = "GCS bucket to use for Terraform remote state"
  value       = google_storage_bucket.terraform_state.name
}
