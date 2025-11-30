use anyhow::{anyhow, bail, Context, Result};
use axum::http::HeaderValue;
use clap::{Parser, Subcommand};
use serde::Deserialize;
use std::{env, net::SocketAddr, path::PathBuf};
use tokio::{net::TcpListener, runtime::Builder as RuntimeBuilder, signal};
use tracing::{info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use url::Url;

use hauski_core::{
    build_app_with_state, load_flags, load_limits, load_models, load_routing, ModelsFile,
};

#[derive(Parser, Debug)]
#[command(name = "hauski", version, about = "HausKI CLI")]
struct Cli {
    /// Mehr Logausgabe
    #[arg(long, short, global = true)]
    verbose: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Modelle verwalten
    Models {
        #[command(subcommand)]
        cmd: ModelsCmd,
    },
    /// Startet den HausKI-Core-Server
    Serve {
        /// Bind-Adresse überschreiben (z. B. 0.0.0.0:8080)
        #[arg(long)]
        bind: Option<String>,
    },
    /// ASR-Werkzeuge
    Asr {
        #[command(subcommand)]
        cmd: AsrCmd,
    },
    /// Audio-Profile (`PipeWire`)
    Audio {
        #[command(subcommand)]
        cmd: AudioCmd,
    },
    /// Konfigurationswerkzeuge
    Config {
        #[command(subcommand)]
        cmd: ConfigCmd,
    },
    /// Führt AI-Assistenten-Playbooks aus
    Assist {
        /// Pfad zur Playbook-Datei
        #[arg(long)]
        playbook: String,
    },
}

#[derive(Subcommand, Debug)]
enum ModelsCmd {
    /// verfügbare Modelle anzeigen (aus configs/models.yml)
    Ls,
    /// Modell herunterladen/registrieren
    Pull { id: String },
}

#[derive(Subcommand, Debug)]
enum AsrCmd {
    /// Datei transkribieren (Stub)
    Transcribe {
        input: String,
        #[arg(long)]
        model: Option<String>,
        #[arg(long)]
        out: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
enum AudioCmd {
    /// Audio-Profil setzen (Stub)
    ProfileSet { profile: String },
}

#[derive(Subcommand, Debug)]
enum ConfigCmd {
    /// Validiert die HausKI-Konfiguration
    Validate {
        /// Pfad zur YAML-Datei
        #[arg(long, default_value = "./configs/hauski.yml")]
        file: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    if cli.verbose {
        eprintln!("verbose on");
    }

    match cli.command {
        Commands::Models { cmd } => match cmd {
            ModelsCmd::Ls => {
                let path = std::env::var("HAUSKI_MODELS")
                    .unwrap_or_else(|_| "./configs/models.yml".to_string());
                let file = load_models(&path)?;
                print_models_table(&file);
            }
            ModelsCmd::Pull { id } => println!("(stub) models pull {id}"),
        },
        Commands::Serve { bind } => {
            run_core_server(bind)?;
        }
        Commands::Asr { cmd } => match cmd {
            AsrCmd::Transcribe { input, model, out } => {
                println!("(stub) asr transcribe {input} --model {model:?} --out {out:?}");
            }
        },
        Commands::Audio { cmd } => match cmd {
            AudioCmd::ProfileSet { profile } => {
                println!("(stub) audio profile set {profile}");
            }
        },
        Commands::Config { cmd } => match cmd {
            ConfigCmd::Validate { file } => {
                validate_config(&file)?;
            }
        },
        Commands::Assist { playbook } => {
            run_playbook(&playbook)?;
        }
    }

    Ok(())
}

fn run_playbook(playbook_path: &str) -> Result<()> {
    let content = std::fs::read_to_string(playbook_path)
        .with_context(|| format!("Could not read playbook file: {playbook_path}"))?;
    let playbook: serde_yaml::Value = serde_yaml::from_str(&content)
        .with_context(|| format!("Could not parse playbook file: {playbook_path}"))?;

    if let Some(steps) = playbook.get("steps").and_then(|s| s.as_sequence()) {
        for (i, step) in steps.iter().enumerate() {
            if let Some(run_cmd) = step.get("run").and_then(|r| r.as_str()) {
                info!("Executing step {}: {}", i + 1, run_cmd);
                let output = std::process::Command::new("sh")
                    .arg("-c")
                    .arg(run_cmd)
                    .output()
                    .with_context(|| format!("Failed to execute command: {run_cmd}"))?;

                if !output.status.success() {
                    bail!(
                        "Step {} failed with status {}:\n{}",
                        i + 1,
                        output.status,
                        String::from_utf8_lossy(&output.stderr)
                    );
                }
            }
        }
    }

    Ok(())
}

// ---- Modelle (nutzt hauski_core::ModelsFile) ----

fn build_table_separator(widths: &[usize; 4]) -> String {
    let mut parts = Vec::with_capacity(widths.len());
    for &width in widths {
        parts.push("-".repeat(width + 2));
    }
    format!("+{}+", parts.join("+"))
}

fn format_table_row(columns: [&str; 4], widths: &[usize; 4]) -> String {
    let mut formatted = String::new();
    formatted.push('|');
    for (idx, column) in columns.iter().enumerate() {
        let width = widths[idx];
        formatted.push(' ');
        formatted.push_str(column);
        let padding = width.saturating_sub(column.chars().count());
        formatted.push_str(&" ".repeat(padding + 1));
        formatted.push('|');
    }
    formatted
}

fn print_models_table(file: &ModelsFile) {
    if file.models.is_empty() {
        println!("Keine Modelle in der Konfiguration gefunden.");
        return;
    }

    const HEADERS: [&str; 4] = ["ID", "Path", "VRAM Min", "Canary"];

    let mut rows: Vec<[String; 4]> = Vec::new();
    let mut widths = HEADERS.map(|header| header.chars().count());

    for model in &file.models {
        let vram = model
            .vram_min_gb
            .map(|value| format!("{value} GB"))
            .unwrap_or_default();
        let canary = model
            .canary
            .map(|value| value.to_string())
            .unwrap_or_default();

        let row = [model.id.clone(), model.path.clone(), vram, canary];

        for (idx, column) in row.iter().enumerate() {
            widths[idx] = widths[idx].max(column.chars().count());
        }

        rows.push(row);
    }

    let separator = build_table_separator(&widths);
    println!("{separator}");
    println!("{}", format_table_row(HEADERS, &widths));
    println!("{separator}");

    for row in &rows {
        println!(
            "{}",
            format_table_row(
                [
                    row[0].as_str(),
                    row[1].as_str(),
                    row[2].as_str(),
                    row[3].as_str(),
                ],
                &widths,
            )
        );
    }

    println!("{separator}");
}

// ---- Konfiguration (YAML) ----

#[derive(Debug, Deserialize)]
struct HauskiConfig {
    index: Option<IndexConfig>,
    budgets: Option<BudgetsConfig>,
    plugins: Option<PluginsConfig>,
}

#[derive(Debug, Deserialize)]
struct IndexConfig {
    path: String,
    provider: ProviderConfig,
}

#[derive(Debug, Deserialize)]
struct ProviderConfig {
    embedder: String,
    model: String,
    url: String,
}

#[derive(Debug, Deserialize)]
struct BudgetsConfig {
    index_topk20_ms: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct PluginsConfig {
    enabled: Option<Vec<String>>,
}

fn validate_config(file: &str) -> Result<()> {
    let expanded_path = shellexpand::full(file)?;
    let path = PathBuf::from(expanded_path.as_ref());
    if !path.exists() {
        bail!("Konfigurationsdatei {} existiert nicht", path.display());
    }

    let content = std::fs::read_to_string(&path).with_context(|| {
        format!(
            "Konfigurationsdatei {} konnte nicht gelesen werden",
            path.display()
        )
    })?;
    let config: HauskiConfig = serde_yaml::from_str(&content)
        .context("Konfiguration konnte nicht als YAML geparst werden")?;

    let index = config
        .index
        .as_ref()
        .ok_or_else(|| anyhow!("index-Block fehlt"))?;

    if index.path.trim().is_empty() {
        bail!("index.path darf nicht leer sein");
    }

    let expanded_index_path = shellexpand::full(&index.path)?;
    let index_path = PathBuf::from(expanded_index_path.as_ref());
    if !index_path.is_absolute() {
        bail!("index.path muss ein absoluter Pfad sein (nach Expansion)");
    }

    if let Some(parent) = index_path.parent() {
        if !parent.exists() {
            eprintln!(
                "warn: Index-Verzeichnis {} existiert noch nicht (wird bei erstem Lauf erstellt)",
                parent.display()
            );
        }
    }

    Url::parse(&index.provider.url).context("index.provider.url ist keine gültige URL")?;

    if index.provider.embedder.trim().is_empty() {
        bail!("index.provider.embedder darf nicht leer sein");
    }

    if index.provider.model.trim().is_empty() {
        bail!("index.provider.model darf nicht leer sein");
    }

    if let Some(budgets) = &config.budgets {
        if budgets.index_topk20_ms.is_none() {
            eprintln!("warn: budgets.index_topk20_ms ist nicht gesetzt");
        }
    } else {
        eprintln!("warn: budgets-Block fehlt");
    }

    if let Some(plugins) = &config.plugins {
        let enabled = plugins
            .enabled
            .as_ref()
            .ok_or_else(|| anyhow!("plugins.enabled fehlt"))?;
        if !enabled.iter().any(|entry| entry == "obsidian_index") {
            bail!("plugins.enabled muss obsidian_index enthalten");
        }
    } else {
        bail!("plugins-Block fehlt");
    }

    println!(
        "Konfiguration gültig: {}\n  index.path: {}\n  provider: {} ({})",
        path.display(),
        index_path.display(),
        index.provider.embedder,
        index.provider.model
    );

    Ok(())
}

fn run_core_server(bind_override: Option<String>) -> Result<()> {
    let runtime = RuntimeBuilder::new_multi_thread()
        .enable_all()
        .build()
        .context("Tokio Runtime konnte nicht erzeugt werden")?;

    runtime.block_on(async move { run_core_server_async(bind_override).await })
}

async fn run_core_server_async(bind_override: Option<String>) -> Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .with(tracing_subscriber::fmt::layer())
        .try_init()
        .ok();

    let limits_path = env::var("HAUSKI_LIMITS").unwrap_or_else(|_| "./policies/limits.yaml".into());
    let models_path = env::var("HAUSKI_MODELS").unwrap_or_else(|_| "./configs/models.yml".into());
    let routing_path =
        env::var("HAUSKI_ROUTING").unwrap_or_else(|_| "./policies/routing.yaml".into());
    let flags_path = env::var("HAUSKI_FLAGS").unwrap_or_else(|_| "./configs/flags.yaml".into());
    let expose_config = env::var("HAUSKI_EXPOSE_CONFIG")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    let allowed_origin =
        env::var("HAUSKI_ALLOWED_ORIGIN").unwrap_or_else(|_| "http://127.0.0.1:8080".into());
    let allowed_origin_header = HeaderValue::from_str(&allowed_origin).map_err(|e| {
        anyhow!(
            "ungültiger Wert für HAUSKI_ALLOWED_ORIGIN '{}': {}",
            allowed_origin,
            e
        )
    })?;

    let (app, state) = build_app_with_state(
        load_limits(limits_path)?,
        load_models(models_path)?,
        load_routing(routing_path)?,
        load_flags(flags_path)?,
        expose_config,
        allowed_origin_header,
    );

    let addr = resolve_bind_addr(bind_override, expose_config)?;
    info!(%addr, expose_config, "starte HausKI-Core (CLI)");
    let listener = TcpListener::bind(addr).await?;
    state.set_ready();
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

fn resolve_bind_addr(bind_override: Option<String>, expose_config: bool) -> Result<SocketAddr> {
    let bind = bind_override
        .or_else(|| env::var("HAUSKI_BIND").ok())
        .unwrap_or_else(|| "127.0.0.1:8080".to_string());
    let addr: SocketAddr = bind
        .parse()
        .map_err(|e| anyhow!("ungültiger Wert für HAUSKI_BIND '{}': {}", bind, e))?;

    let is_loopback = match addr.ip() {
        std::net::IpAddr::V4(v4) => v4.is_loopback(),
        std::net::IpAddr::V6(v6) => v6.is_loopback(),
    };

    if expose_config && !is_loopback {
        bail!("HAUSKI_EXPOSE_CONFIG erfordert Loopback-Bind; nutze z. B. 127.0.0.1:<port>");
    }

    if !expose_config && !is_loopback {
        warn!(
            "Binde an nicht-Loopback-Adresse ({}); EXPOSE_CONFIG=false",
            addr
        );
    }

    Ok(addr)
}

async fn shutdown_signal() {
    let ctrl_c = async {
        match signal::ctrl_c().await {
            Ok(()) => {
                info!("Ctrl+C received");
            }
            Err(e) => {
                warn!("Failed to install Ctrl+C handler: {}", e);
                // Keep waiting indefinitely if signal handler fails
                std::future::pending::<()>().await
            }
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match signal::unix::signal(signal::unix::SignalKind::terminate()) {
            Ok(mut sig) => {
                sig.recv().await;
                info!("SIGTERM received");
            }
            Err(e) => {
                warn!("Failed to install SIGTERM handler: {}", e);
                // Keep waiting indefinitely if signal handler fails
                std::future::pending::<()>().await
            }
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }

    info!("Shutdown signal received, shutting down");
}

#[cfg(test)]
mod tests {
    use super::*;
    use hauski_core::ModelEntry;

    #[test]
    fn print_models_table_handles_empty_list() {
        let models = ModelsFile { models: vec![] };
        print_models_table(&models);
    }

    #[test]
    fn print_models_table_handles_mixed_list() {
        let models = ModelsFile {
            models: vec![
                ModelEntry {
                    id: "test-model-1".into(),
                    path: "/path/to/model-1".into(),
                    vram_min_gb: Some(4),
                    canary: Some(true),
                },
                ModelEntry {
                    id: "test-model-2".into(),
                    path: "/path/to/model-2".into(),
                    vram_min_gb: None,
                    canary: None,
                },
            ],
        };
        print_models_table(&models);
    }
}
