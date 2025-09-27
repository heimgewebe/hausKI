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
                println!(
                    "(stub) asr transcribe {input} --model {:?} --out {:?}",
                    model, out
                )
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
    use tabled::{Table, Tabled};

    #[derive(Tabled)]
    struct Row<'a> {
        id: &'a str,
        path: &'a str,
        #[tabled(rename = "VRAM Min")]
        vram: String,
        canary: String,
    }

    let rows: Vec<Row> = file
        .models
        .iter()
        .map(|model| Row {
            id: &model.id,
            path: &model.path,
            vram: model
                .vram_min_gb
                .map(|value| format!("{value} GB"))
                .unwrap_or_default(),
            canary: model
                .canary
                .map(|value| value.to_string())
                .unwrap_or_default(),
        })
        .collect();

    let table = Table::new(rows).to_string();
    println!("{table}");
}
