use std::sync::Arc;

use crate::config::Settings;

#[derive(Clone)]
pub struct AppState {
    pub settings: Arc<Settings>,
}
