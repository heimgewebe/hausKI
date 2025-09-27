use clap::{Parser, Subcommand};

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
            ModelsCmd::Ls => println!("(stub) models ls"),
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
            AudioCmd::ProfileSet { profile } => println!("(stub) audio profile set {profile}"),
        },
    }

    Ok(())
}
