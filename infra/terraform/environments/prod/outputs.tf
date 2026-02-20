output "artifact_registry_repository" {
  value       = google_artifact_registry_repository.backend.id
  description = "Artifact Registry repository ID"
}

output "backend_service_url" {
  value       = google_cloud_run_v2_service.api.uri
  description = "Public URL for the backend API"
}

output "frontend_bucket_name" {
  value       = google_storage_bucket.frontend.name
  description = "Bucket hosting built frontend assets"
}

output "frontend_service_url" {
  value       = google_cloud_run_v2_service.frontend.uri
  description = "Public URL for the frontend web app"
}
