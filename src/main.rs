use clap::{Parser, Subcommand};
use tracing::info;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

mod config;
mod correction;
mod daemon;
mod engine;
#[cfg(target_os = "linux")]
mod gui;
mod input;
mod output;
mod panic_handler;
mod platform;
mod queue;
#[cfg(target_os = "linux")]
mod tray;
mod vad;
mod vocabulary;

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

        /// Disable system tray icon
        #[arg(long)]
        no_tray: bool,
    },

    /// Open preferences GUI
    Preferences,

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

        /// Enable translation to English (use --translate or --no-translate)
        #[arg(long, action = clap::ArgAction::Set)]
        translate: Option<bool>,

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

/// Guard that must be kept alive for file logging to work
struct LogGuard {
    _guard: Option<tracing_appender::non_blocking::WorkerGuard>,
}

fn init_logging(verbose: bool, foreground: bool) -> LogGuard {
    let filter = if verbose {
        EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new("openhush=debug,whisper_rs=info"))
    } else {
        EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new("openhush=info,whisper_rs=warn"))
    };

    if foreground {
        // Foreground mode: log to stdout with pretty formatting
        tracing_subscriber::registry()
            .with(filter)
            .with(fmt::layer().with_target(true))
            .init();

        LogGuard { _guard: None }
    } else {
        // Daemon mode: log to file with rotation
        let log_dir = config::Config::data_dir()
            .map(|d| d.to_path_buf())
            .unwrap_or_else(|_| std::env::temp_dir());

        // Create log directory if needed
        if let Err(e) = std::fs::create_dir_all(&log_dir) {
            eprintln!("Warning: Failed to create log directory {}: {}", log_dir.display(), e);
        }

        // Daily rotation, keep logs for 7 days
        let file_appender = RollingFileAppender::new(Rotation::DAILY, &log_dir, "openhush.log");

        let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

        tracing_subscriber::registry()
            .with(filter)
            .with(
                fmt::layer()
                    .with_target(true)
                    .with_ansi(false) // No colors in file
                    .with_writer(non_blocking),
            )
            .init();

        // Also log to stderr in daemon mode for immediate feedback
        // (This won't work with the current setup, but the file logging is the important part)

        LogGuard {
            _guard: Some(guard),
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Install panic handler first, before anything else
    panic_handler::install();

    let cli = Cli::parse();

    // Determine if we're running in foreground mode for logging
    let foreground_mode = match &cli.command {
        Commands::Start { foreground, .. } => *foreground,
        _ => true, // All other commands run in foreground
    };

    // Initialize logging (keep guard alive for the duration of the program)
    let _log_guard = init_logging(cli.verbose, foreground_mode);

    match cli.command {
        Commands::Start {
            foreground,
            no_tray,
        } => {
            info!("Starting OpenHush daemon...");
            daemon::run(foreground, !no_tray).await?;
        }

        Commands::Preferences => {
            info!("Opening preferences...");
            #[cfg(target_os = "linux")]
            {
                gui::run_preferences()?;
            }
            #[cfg(not(target_os = "linux"))]
            {
                eprintln!("Preferences GUI is only available on Linux in this release.");
                eprintln!("Use 'openhush config --show' to view current settings.");
            }
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
            translate,
            llm,
            show,
        } => {
            if show {
                config::show()?;
            } else {
                config::update(hotkey, model, language, translate, llm)?;
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
