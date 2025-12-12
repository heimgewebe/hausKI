use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use utoipa::ToSchema;

use crate::AppState;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Plugin {
    pub id: String,
    pub name: String,
    pub description: String,
    pub version: String,
    pub enabled: bool,
}

pub struct PluginRegistry {
    plugins: HashMap<String, Plugin>,
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self {
            plugins: HashMap::new(),
        }
    }

    pub fn register(&mut self, plugin: Plugin) {
        self.plugins.insert(plugin.id.clone(), plugin);
    }

    pub fn get(&self, id: &str) -> Option<Plugin> {
        self.plugins.get(id).cloned()
    }

    pub fn list(&self) -> Vec<Plugin> {
        let mut list: Vec<Plugin> = self.plugins.values().cloned().collect();
        list.sort_by(|a, b| a.id.cmp(&b.id));
        list
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
    Json(state.plugins().list())
}

#[utoipa::path(
    get,
    path = "/plugins/{id}",
    responses(
        (status = 200, description = "Details of a specific plugin", body = Plugin),
        (status = 404, description = "Plugin not found")
    ),
    params(
        ("id" = String, Path, description = "Plugin ID")
    ),
    tag = "plugins"
)]
pub async fn get_plugin_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Plugin>, StatusCode> {
    if let Some(plugin) = state.plugins().get(&id) {
        Ok(Json(plugin))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}
