use clap::{Parser, Subcommand};
use serde::Deserialize;

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

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    if cli.verbose {
        eprintln!("verbose on");
    }

    match cli.command {
        Commands::Models { cmd } => match cmd {
            ModelsCmd::Ls => {
                let path = std::env::var("HAUSKI_MODELS")
                    .unwrap_or_else(|_| "./configs/models.yml".to_string());
                let content = std::fs::read_to_string(&path)?;
                let file: ModelsFile = serde_yaml::from_str(&content)?;
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
    }

    Ok(())
}

#[derive(Debug, Deserialize)]
struct ModelsFile {
    models: Vec<ModelEntry>,
}

#[derive(Debug, Deserialize)]
struct ModelEntry {
    id: String,
    path: String,
    vram_min_gb: Option<u64>,
    canary: Option<bool>,
}

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
