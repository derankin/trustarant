variable "project_name" {
  description = "Display name of the GCP project"
  type        = string
  default     = "cleanplated"
}

variable "project_id_prefix" {
  description = "Prefix for globally unique project IDs"
  type        = string
  default     = "cleanplated"
}

variable "billing_account" {
  description = "Billing account ID (format: XXXXXX-XXXXXX-XXXXXX)"
  type        = string
}

variable "org_id" {
  description = "Optional organization ID"
  type        = string
  default     = ""
}

variable "folder_id" {
  description = "Optional folder ID"
  type        = string
  default     = ""
}

variable "region" {
  description = "Default region for deployed services"
  type        = string
  default     = "us-west1"
}

variable "state_bucket_location" {
  description = "Location for Terraform state bucket"
  type        = string
  default     = "US"
}
