use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};
use sysinfo::{CpuRefreshKind, MemoryRefreshKind, RefreshKind, System};
use tokio::time::{sleep, Duration};
use utoipa::ToSchema;

use crate::AppState;

#[derive(Serialize, Deserialize, Clone, Debug, Default, ToSchema)]
pub struct SystemSignals {
    /// Global CPU load in percent (0.0 - 100.0), smoothed.
    pub cpu_load: f32,
    /// Memory pressure in percent (0.0 - 100.0), smoothed.
    pub memory_pressure: f32,
    /// Whether an NVIDIA GPU is detected available.
    pub gpu_available: bool,
}

/// Helper to manage system monitoring in the background.
#[derive(Clone)]
pub struct SystemMonitor {
    signals: Arc<RwLock<SystemSignals>>,
}

impl SystemMonitor {
    pub fn new() -> Self {
        let signals = Arc::new(RwLock::new(SystemSignals::default()));
        let signals_clone = signals.clone();

        tokio::spawn(async move {
            let mut sys = System::new_with_specifics(
                RefreshKind::new()
                    .with_cpu(CpuRefreshKind::new().with_cpu_usage())
                    .with_memory(MemoryRefreshKind::everything()),
            );

            // Check GPU availability once (heuristic)
            let gpu_available = check_gpu_availability();

            // Initial refresh
            sys.refresh_cpu();
            sys.refresh_memory();
            // Wait a bit for CPU usage to have a delta
            sleep(Duration::from_millis(200)).await;
            sys.refresh_cpu();

            // Initialize values
            {
                let mut guard = signals_clone.write().unwrap();
                guard.gpu_available = gpu_available;
                guard.cpu_load = sys.global_cpu_info().cpu_usage();
                let used = sys.used_memory() as f64;
                let total = sys.total_memory() as f64;
                guard.memory_pressure = if total > 0.0 {
                    (used / total * 100.0) as f32
                } else {
                    0.0
                };
            }

            let alpha = 0.1; // Smoothing factor (EWMA)

            loop {
                sleep(Duration::from_secs(2)).await;

                // Refresh system stats
                sys.refresh_cpu();
                sys.refresh_memory();

                let current_cpu = sys.global_cpu_info().cpu_usage();
                let used = sys.used_memory() as f64;
                let total = sys.total_memory() as f64;
                let current_mem = if total > 0.0 {
                    (used / total * 100.0) as f32
                } else {
                    0.0
                };

                if let Ok(mut guard) = signals_clone.write() {
                    // Exponential Moving Average
                    guard.cpu_load = alpha * current_cpu + (1.0 - alpha) * guard.cpu_load;
                    guard.memory_pressure =
                        alpha * current_mem + (1.0 - alpha) * guard.memory_pressure;
                    guard.gpu_available = gpu_available;
                }
            }
        });

        Self { signals }
    }

    pub fn get_signals(&self) -> SystemSignals {
        self.signals.read().unwrap().clone()
    }
}

fn check_gpu_availability() -> bool {
    // Simple check for nvidia-smi
    std::process::Command::new("nvidia-smi")
        .arg("-L")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

// Handler
#[utoipa::path(
    get,
    path = "/system/signals",
    responses(
        (status = 200, description = "System signals", body = SystemSignals)
    ),
    tag = "system"
)]
pub async fn system_signals_handler(State(state): State<AppState>) -> Json<SystemSignals> {
    Json(state.system_monitor().get_signals())
}
