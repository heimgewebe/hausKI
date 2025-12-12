use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Instant;

use crate::AppState;

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Plugin {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub enabled: bool,
    // Future: capabilities, permissions, etc.
}

#[derive(Debug, Clone, Default)]
pub struct PluginRegistry {
    plugins: Arc<RwLock<HashMap<String, Plugin>>>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self {
            plugins: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn register(&self, plugin: Plugin) {
        let mut plugins = self.plugins.write().unwrap();
        plugins.insert(plugin.id.clone(), plugin);
    }

    pub fn list(&self) -> Vec<Plugin> {
        let plugins = self.plugins.read().unwrap();
        plugins.values().cloned().collect()
    }

    pub fn get(&self, id: &str) -> Option<Plugin> {
        let plugins = self.plugins.read().unwrap();
        plugins.get(id).cloned()
    }
}

#[utoipa::path(
    get,
    path = "/plugins",
    responses(
        (status = 200, description = "List of all registered plugins", body = Vec<Plugin>)
    ),
    tag = "plugins"
)]
pub async fn list_plugins_handler(State(state): State<AppState>) -> Json<Vec<Plugin>> {
    let started = Instant::now();
    let plugins = state.plugins().list();
    state.record_http_observation(axum::http::Method::GET, "/plugins", StatusCode::OK, started);
    Json(plugins)
}

#[utoipa::path(
    get,
    path = "/plugins/{id}",
    responses(
        (status = 200, description = "Plugin details", body = Plugin),
        (status = 404, description = "Plugin not found")
    ),
    params(
        ("id" = String, Path, description = "Plugin identifier")
    ),
    tag = "plugins"
)]
pub async fn get_plugin_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Plugin>, StatusCode> {
    let started = Instant::now();
    if let Some(plugin) = state.plugins().get(&id) {
        state.record_http_observation(
            axum::http::Method::GET,
            "/plugins/{id}",
            StatusCode::OK,
            started,
        );
        Ok(Json(plugin))
    } else {
        state.record_http_observation(
            axum::http::Method::GET,
            "/plugins/{id}",
            StatusCode::NOT_FOUND,
            started,
        );
        Err(StatusCode::NOT_FOUND)
    }
}
