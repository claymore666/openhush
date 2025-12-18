use clap::{Parser, Subcommand};
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

mod config;
mod daemon;
mod platform;

#[derive(Parser)]
#[command(name = "openhush")]
#[command(author, version, about = "Voice-to-text whisper keyboard", long_about = None)]
struct Cli {
    /// Enable verbose logging
    #[arg(short, long, global = true)]
    verbose: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the daemon (default mode)
    Start {
        /// Run in foreground instead of daemonizing
        #[arg(short, long)]
        foreground: bool,
    },

    /// Stop the running daemon
    Stop,

    /// Check daemon status
    Status,

    /// Configure settings
    Config {
        /// Set the hotkey (e.g., "ctrl_r", "f12")
        #[arg(long)]
        hotkey: Option<String>,

        /// Set the Whisper model (tiny, base, small, medium, large-v3)
        #[arg(long)]
        model: Option<String>,

        /// Set the language (auto, en, de, etc.)
        #[arg(long)]
        language: Option<String>,

        /// Enable/disable LLM correction
        #[arg(long)]
        llm: Option<String>,

        /// Show current configuration
        #[arg(long)]
        show: bool,
    },

    /// Manage Whisper models
    Model {
        #[command(subcommand)]
        action: ModelAction,
    },

    /// One-shot transcription from file
    Transcribe {
        /// Audio file to transcribe
        file: String,

        /// Output format (text, json)
        #[arg(short, long, default_value = "text")]
        format: String,
    },
}

#[derive(Subcommand)]
enum ModelAction {
    /// Download a model
    Download {
        /// Model name (tiny, base, small, medium, large-v3)
        name: String,
    },

    /// List available models
    List,

    /// Remove a downloaded model
    Remove {
        /// Model name
        name: String,
    },
}

fn init_logging(verbose: bool) {
    let filter = if verbose {
        EnvFilter::new("openhush=debug,whisper_rs=info")
    } else {
        EnvFilter::new("openhush=info,whisper_rs=warn")
    };

    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer())
        .init();
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    init_logging(cli.verbose);

    match cli.command {
        Commands::Start { foreground } => {
            info!("Starting OpenHush daemon...");
            daemon::run(foreground).await?;
        }

        Commands::Stop => {
            info!("Stopping OpenHush daemon...");
            daemon::stop().await?;
        }

        Commands::Status => {
            daemon::status().await?;
        }

        Commands::Config {
            hotkey,
            model,
            language,
            llm,
            show,
        } => {
            if show {
                config::show()?;
            } else {
                config::update(hotkey, model, language, llm)?;
            }
        }

        Commands::Model { action } => match action {
            ModelAction::Download { name } => {
                info!("Downloading model: {}", name);
                // TODO: Implement model download
                println!("Model download not yet implemented");
            }
            ModelAction::List => {
                // TODO: Implement model listing
                println!("Available models:");
                println!("  tiny      - 75MB,  fastest, lowest accuracy");
                println!("  base      - 142MB, fast");
                println!("  small     - 466MB, balanced");
                println!("  medium    - 1.5GB, good accuracy");
                println!("  large-v3  - 3GB,   best accuracy");
            }
            ModelAction::Remove { name } => {
                info!("Removing model: {}", name);
                // TODO: Implement model removal
                println!("Model removal not yet implemented");
            }
        },

        Commands::Transcribe { file, format } => {
            info!("Transcribing: {}", file);
            // TODO: Implement one-shot transcription
            println!(
                "One-shot transcription not yet implemented (file: {}, format: {})",
                file, format
            );
        }
    }

    Ok(())
}
