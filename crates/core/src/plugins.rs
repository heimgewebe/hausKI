use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard},
    time::Instant,
};

use crate::AppState;
use tracing::warn;

const PLUGIN_BY_ID_PATH: &str = "/plugins/{id}";

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
        let mut plugins = self.write_plugins("register");
        plugins.insert(plugin.id.clone(), plugin);
    }

    pub fn list(&self) -> Vec<Plugin> {
        let plugins = self.read_plugins("list");
        plugins.values().cloned().collect()
    }

    pub fn get(&self, id: &str) -> Option<Plugin> {
        let plugins = self.read_plugins("get");
        plugins.get(id).cloned()
    }

    fn read_plugins(&self, op: &str) -> RwLockReadGuard<'_, HashMap<String, Plugin>> {
        self.plugins.read().unwrap_or_else(|poisoned| {
            warn!(
                operation = op,
                "RwLock poisoned, recovered via into_inner()"
            );
            poisoned.into_inner()
        })
    }

    fn write_plugins(&self, op: &str) -> RwLockWriteGuard<'_, HashMap<String, Plugin>> {
        self.plugins.write().unwrap_or_else(|poisoned| {
            warn!(
                operation = op,
                "RwLock poisoned, recovered via into_inner()"
            );
            poisoned.into_inner()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_poison_recovery() {
        let registry = Arc::new(PluginRegistry::new());
        let registry_clone = registry.clone();

        // 1. Poison the lock by panicking while holding write guard
        let handle = thread::spawn(move || {
            let _guard = registry_clone.write_plugins("test_panic");
            panic!("Oops");
        });
        let _ = handle.join(); // This will return Err because of panic

        // 2. Verify we can still access it (recovery works)
        let plugins = registry.list();
        assert!(plugins.is_empty(), "Should recover empty state");

        // 3. Verify we can still write to it
        registry.register(Plugin {
            id: "test".into(),
            name: "Test".into(),
            version: "0.1".into(),
            description: "Desc".into(),
            enabled: true,
        });

        assert!(registry.get("test").is_some());
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
            PLUGIN_BY_ID_PATH,
            StatusCode::OK,
            started,
        );
        Ok(Json(plugin))
    } else {
        state.record_http_observation(
            axum::http::Method::GET,
            PLUGIN_BY_ID_PATH,
            StatusCode::NOT_FOUND,
            started,
        );
        Err(StatusCode::NOT_FOUND)
    }
}
