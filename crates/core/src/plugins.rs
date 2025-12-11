use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone, Serialize, Deserialize)]
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
