use std::hash::Hash;

use prometheus_client::encoding::EncodeLabelSet;
use prometheus_client::metrics::counter::Counter;
use prometheus_client::metrics::family::Family;

/// Label-Set für Memory-Metriken.
///
/// WICHTIG: Für `Family<L, M>` müssen die Labels `Eq + Hash` implementieren
/// und `EncodeLabelSet` liefern.
#[derive(Clone, Debug, PartialEq, Eq, Hash, EncodeLabelSet)]
pub struct MemoryLabels {
    pub namespace: String,
    pub layer: String,
}

/// Minimaler Memory-Store nur für Metriken (A1).
///
/// A2 ersetzt/ergänzt dies um SQLite, TTL, Janitor & CRUD.
#[derive(Default)]
pub struct MemoryStore {
    ops_total: Family<MemoryLabels, Counter>,
}

impl MemoryStore {
    /// Erzeugt einen leeren Store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Zugriff auf die Metrik-Family zur Registrierung im Registry.
    pub fn ops_family(&self) -> &Family<MemoryLabels, Counter> {
        &self.ops_total
    }

    /// Erhöht einen Zähler für eine Operation in einem Namespace/Layer.
    ///
    /// Beispielverwendung im Core (A2): bei `set/get/evict` aufrufen.
    pub fn touch<N, L>(&self, namespace: N, layer: L)
    where
        N: Into<String>,
        L: Into<String>,
    {
        let labels = MemoryLabels {
            namespace: namespace.into(),
            layer: layer.into(),
        };
        let c = self.ops_total.get_or_create(&labels);
        c.inc();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_encode_ok() {
        // Smoke-Test: compile-level assurance; runtime just ensures trait is callable.
        let labels = MemoryLabels {
            namespace: "default".to_string(),
            layer: "short_term".to_string(),
        };
        // no-op: compile-time check is what we want here.
        let _ = labels;
    }

    #[test]
    fn family_get_or_create_and_inc() {
        let store = MemoryStore::new();
        store.touch("ns", "layer");
        // If this compiles & runs without panic, Family<Labels, Counter> plumbing is good.
    }
}
