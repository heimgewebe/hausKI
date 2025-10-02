use anyhow::{anyhow, bail, Context, Result};
use clap::{Parser, Subcommand};
use serde::Deserialize;
use std::path::PathBuf;
use url::Url;

use hauski_core::{load_models, ModelsFile};

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
    /// ASR-Werkzeuge
    Asr {
        #[command(subcommand)]
        cmd: AsrCmd,
    },
    /// Audio-Profile (PipeWire)
    Audio {
        #[command(subcommand)]
        cmd: AudioCmd,
    },
    /// Konfigurationswerkzeuge
    Config {
        #[command(subcommand)]
        cmd: ConfigCmd,
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
        Commands::Asr { cmd } => match cmd {
            AsrCmd::Transcribe { input, model, out } => {
                println!("(stub) asr transcribe {input} --model {model:?} --out {out:?}")
            }
        },
        Commands::Audio { cmd } => match cmd {
            AudioCmd::ProfileSet { profile } => {
                println!("(stub) audio profile set {profile}")
            }
        },
        Commands::Config { cmd } => match cmd {
            ConfigCmd::Validate { file } => {
                validate_config(file)?;
            }
        },
    }

    Ok(())
}

// ---- Modelle (nutzt hauski_core::ModelsFile) ----

fn print_models_table(file: &ModelsFile) {
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

    fn build_separator(widths: &[usize; 4]) -> String {
        let mut parts = Vec::with_capacity(widths.len());
        for &width in widths {
            parts.push("-".repeat(width + 2));
        }
        format!("+{}+", parts.join("+"))
    }

    fn format_row(columns: [&str; 4], widths: &[usize; 4]) -> String {
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

    let separator = build_separator(&widths);
    println!("{separator}");
    println!("{}", format_row(HEADERS, &widths));
    println!("{separator}");

    for row in &rows {
        println!(
            "{}",
            format_row(
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

    if !rows.is_empty() {
        println!("{separator}");
    }
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

fn validate_config(file: String) -> Result<()> {
    let expanded_path = shellexpand::full(&file)?;
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
