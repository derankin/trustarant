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

output "ingestion_scheduler_job" {
  value       = var.enable_ingestion_scheduler ? google_cloud_scheduler_job.ingestion_refresh[0].name : null
  description = "Cloud Scheduler job name that triggers ingestion refresh"
}

output "ingestion_cloud_run_job" {
  value       = google_cloud_run_v2_job.ingestion.name
  description = "Cloud Run Job that performs ingestion refreshes"
}

output "database_url_secret" {
  value       = google_secret_manager_secret.database_url.secret_id
  description = "Secret ID storing the PostgreSQL/Neon DATABASE_URL"
}
