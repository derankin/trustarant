variable "project_id" {
  description = "GCP project ID"
  type        = string
}

variable "region" {
  description = "Primary deployment region"
  type        = string
  default     = "us-west1"
}

variable "backend_image" {
  description = "Container image for the Rust backend Cloud Run service"
  type        = string
  default     = "us-docker.pkg.dev/cloudrun/container/hello"
}

variable "frontend_bucket_name" {
  description = "Optional bucket name for static frontend hosting"
  type        = string
  default     = ""
}

variable "frontend_image" {
  description = "Container image for the frontend Cloud Run service"
  type        = string
  default     = "us-docker.pkg.dev/cloudrun/container/hello"
}

variable "disable_backend_invoker_iam_check" {
  description = "When true, Cloud Run allows unauthenticated access without allUsers IAM binding"
  type        = bool
  default     = false
}

variable "invoker_members" {
  description = "IAM principals (user:, group:, serviceAccount:) to grant roles/run.invoker"
  type        = list(string)
  default     = []
}

variable "allow_public_frontend_bucket" {
  description = "When true, grants allUsers objectViewer on the frontend bucket"
  type        = bool
  default     = false
}

variable "disable_frontend_invoker_iam_check" {
  description = "When true, frontend Cloud Run allows unauthenticated access without allUsers IAM"
  type        = bool
  default     = true
}
