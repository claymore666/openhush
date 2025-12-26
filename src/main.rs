use clap::{Parser, Subcommand};
use tracing::info;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

mod config;
mod correction;
mod daemon;
#[cfg(target_os = "linux")]
mod dbus;
mod engine;
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
mod gui;
mod input;
mod output;
mod panic_handler;
mod platform;
mod queue;
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
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

        /// Override model (tiny, base, small, medium, large-v3)
        #[arg(short, long)]
        model: Option<String>,
    },

    /// Control recording on a running daemon (Linux only, requires D-Bus)
    #[cfg(target_os = "linux")]
    Recording {
        #[command(subcommand)]
        action: RecordingAction,
    },
}

/// Recording control actions (sent to daemon via D-Bus)
#[cfg(target_os = "linux")]
#[derive(Subcommand)]
enum RecordingAction {
    /// Start recording audio
    Start,

    /// Stop recording audio
    Stop,

    /// Toggle recording state
    Toggle,

    /// Show current recording status
    Status,
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

fn init_logging(verbose: bool, foreground: bool, config_level: Option<&str>) -> LogGuard {
    // Priority: RUST_LOG env > --verbose flag > config file > default
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        let level = if verbose {
            "debug"
        } else {
            config_level.unwrap_or("info")
        };
        // Set openhush to the configured level, whisper_rs one level quieter
        let whisper_level = match level {
            "trace" | "debug" => "info",
            "info" => "warn",
            _ => "error",
        };
        EnvFilter::new(format!("openhush={},whisper_rs={}", level, whisper_level))
    });

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
            eprintln!(
                "Warning: Failed to create log directory {}: {}",
                log_dir.display(),
                e
            );
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

    // Load config early to get log level (use default if config fails)
    let config_log_level = config::Config::load().ok().map(|c| c.logging.level);

    // Initialize logging (keep guard alive for the duration of the program)
    let _log_guard = init_logging(cli.verbose, foreground_mode, config_log_level.as_deref());

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
            gui::run_preferences()?;
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
                use engine::whisper::{
                    download_model, format_size, model_size_bytes, WhisperModel,
                };
                use std::io::Write;

                let model: WhisperModel = name.parse().map_err(|()| {
                    anyhow::anyhow!(
                        "Unknown model '{}'. Available: tiny, base, small, medium, large-v3",
                        name
                    )
                })?;

                println!(
                    "Downloading {} ({})...",
                    model.filename(),
                    format_size(model_size_bytes(model))
                );

                let mut last_percent = 0;
                let path = download_model(model, |downloaded, total| {
                    let percent = ((downloaded as f64 / total as f64) * 100.0) as u32;
                    if percent > last_percent {
                        last_percent = percent;
                        print!(
                            "\r  Progress: {}% ({} / {})",
                            percent,
                            format_size(downloaded),
                            format_size(total)
                        );
                        let _ = std::io::stdout().flush();
                    }
                })
                .await?;

                println!("\nDownloaded to: {}", path.display());
            }
            ModelAction::List => {
                use engine::whisper::{
                    all_models, format_size, is_model_downloaded, model_size_bytes,
                };

                println!("Available Whisper models:\n");
                println!(
                    "  {:<12} {:<10} {:<10} Description",
                    "Model", "Size", "Status"
                );
                println!("  {}", "-".repeat(60));

                for model in all_models() {
                    let name = format!("{:?}", model).to_lowercase();
                    let size = format_size(model_size_bytes(model));
                    let status = if is_model_downloaded(model) {
                        "✓ local"
                    } else {
                        "remote"
                    };
                    let desc = match model {
                        engine::whisper::WhisperModel::Tiny => "Fastest, lowest accuracy",
                        engine::whisper::WhisperModel::Base => "Fast, good for simple audio",
                        engine::whisper::WhisperModel::Small => "Balanced speed/accuracy",
                        engine::whisper::WhisperModel::Medium => "Good accuracy, slower",
                        engine::whisper::WhisperModel::LargeV3 => "Best accuracy, slowest",
                    };
                    println!("  {:<12} {:<10} {:<10} {}", name, size, status, desc);
                }

                println!("\nUse 'openhush model download <name>' to download a model.");
            }
            ModelAction::Remove { name } => {
                use engine::whisper::{remove_model, WhisperModel};

                let model: WhisperModel = name.parse().map_err(|()| {
                    anyhow::anyhow!(
                        "Unknown model '{}'. Available: tiny, base, small, medium, large-v3",
                        name
                    )
                })?;

                remove_model(model)?;
                println!("Removed model: {}", model.filename());
            }
        },

        Commands::Transcribe {
            file,
            format,
            model: model_override,
        } => {
            use std::path::Path;
            use std::time::Instant;

            let file_path = Path::new(&file);

            // Validate file exists
            if !file_path.exists() {
                anyhow::bail!("File not found: {}", file);
            }

            // Load config for model and language settings
            let config = config::Config::load().unwrap_or_default();

            // Load audio file
            info!("Loading audio file: {}", file);
            let start_load = Instant::now();
            let audio = input::load_wav_file(file_path, config.audio.resampling_quality)?;
            let load_time = start_load.elapsed();

            println!(
                "Loaded: {:.2}s audio ({} samples) in {:.0}ms",
                audio.duration_secs(),
                audio.samples.len(),
                load_time.as_millis()
            );

            // Initialize Whisper engine
            let data_dir = config::Config::data_dir()?;
            let model_name = model_override
                .as_deref()
                .unwrap_or_else(|| config.transcription.effective_model());
            let model: engine::whisper::WhisperModel = model_name.parse().map_err(|()| {
                anyhow::anyhow!(
                    "Unknown model '{}'. Available: tiny, base, small, medium, large-v3",
                    model_name
                )
            })?;
            let model_path = data_dir.join("models").join(model.filename());

            if !model_path.exists() {
                anyhow::bail!(
                    "Model not found: {}\nRun: openhush model download {}",
                    model_path.display(),
                    model_name
                );
            }

            println!(
                "Loading model: {} (GPU: {})",
                model.filename(),
                config.transcription.device.to_lowercase() != "cpu"
            );

            let start_model = Instant::now();
            let engine = engine::whisper::WhisperEngine::new(
                &model_path,
                &config.transcription.language,
                config.transcription.translate,
                config.transcription.device.to_lowercase() != "cpu",
            )?;
            let model_time = start_model.elapsed();
            println!("Model loaded in {:.0}ms", model_time.as_millis());

            // Transcribe
            println!("Transcribing...");
            let start_transcribe = Instant::now();
            let result = engine.transcribe(&audio)?;
            let transcribe_time = start_transcribe.elapsed();

            // Calculate real-time factor (RTF)
            let rtf = transcribe_time.as_secs_f32() / audio.duration_secs();

            match format.as_str() {
                "json" => {
                    // JSON output for programmatic use
                    let json = serde_json::json!({
                        "text": result.text,
                        "language": result.language,
                        "duration_ms": result.duration_ms,
                        "audio_duration_secs": audio.duration_secs(),
                        "transcription_time_ms": transcribe_time.as_millis() as u64,
                        "real_time_factor": rtf,
                        "model": format!("{:?}", model).to_lowercase(),
                    });
                    println!("{}", serde_json::to_string_pretty(&json)?);
                }
                _ => {
                    // Text output (default)
                    println!("\n--- Transcription ---");
                    println!("{}", result.text);
                    println!("---");
                    println!(
                        "\nTime: {:.0}ms (RTF: {:.3}x)",
                        transcribe_time.as_millis(),
                        rtf
                    );
                }
            }
        }

        #[cfg(target_os = "linux")]
        Commands::Recording { action } => {
            use dbus::DbusClient;

            let client = match DbusClient::connect().await {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("Failed to connect to D-Bus: {}", e);
                    eprintln!("Is the daemon running? Try: openhush start");
                    std::process::exit(1);
                }
            };

            if !client.is_daemon_running().await {
                eprintln!("Daemon is not running. Start it with: openhush start");
                std::process::exit(1);
            }

            match action {
                RecordingAction::Start => {
                    client.start_recording().await?;
                    println!("Recording started");
                }
                RecordingAction::Stop => {
                    client.stop_recording().await?;
                    println!("Recording stopped");
                }
                RecordingAction::Toggle => {
                    client.toggle_recording().await?;
                    let status = client.get_status().await?;
                    println!("Recording toggled: {}", status);
                }
                RecordingAction::Status => {
                    let status = client.get_status().await?;
                    let queue = client.queue_depth().await?;
                    let version = client.version().await?;
                    println!("Status: {}", status);
                    println!("Queue depth: {}", queue);
                    println!("Version: {}", version);
                }
            }
        }
    }

    Ok(())
}
