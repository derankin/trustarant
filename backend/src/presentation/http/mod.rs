pub mod handlers;
pub mod routes;

use std::sync::Arc;

use crate::application::services::{DirectoryService, IngestionService};

#[derive(Clone)]
pub struct AppState {
    pub directory_service: Arc<DirectoryService>,
    pub ingestion_service: Arc<IngestionService>,
}
