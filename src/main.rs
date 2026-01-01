use clap::{Parser, Subcommand};
use tracing::info;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

mod api;
mod config;
mod context;
mod correction;
mod daemon;
#[cfg(target_os = "linux")]
mod dbus;
#[cfg(feature = "diarization")]
mod diarization;
mod download_queue;
mod engine;
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
mod gui;
mod input;
#[cfg(any(target_os = "macos", target_os = "windows"))]
mod ipc;
mod output;
mod panic_handler;
mod platform;
mod queue;
#[cfg(target_os = "linux")]
mod recording;
mod secrets;
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
mod service;
mod summarization;
mod translation;
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

    /// Run the first-run setup wizard
    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    Setup,

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

        /// Enable Whisper's built-in translation to English (--translate/--no-translate)
        #[arg(long, action = clap::ArgAction::Set)]
        translate: Option<bool>,

        /// Enable/disable LLM correction
        #[arg(long)]
        llm: Option<String>,

        /// Enable/disable real-time translation (m2m100 or ollama)
        #[arg(long, action = clap::ArgAction::Set)]
        translation: Option<bool>,

        /// Set translation engine (m2m100 or ollama)
        #[arg(long)]
        translation_engine: Option<String>,

        /// Set translation target language (e.g., "de", "fr", "es")
        #[arg(long)]
        translation_target: Option<String>,

        /// Show current configuration
        #[arg(long)]
        show: bool,
    },

    /// Manage Whisper models
    Model {
        #[command(subcommand)]
        action: ModelAction,
    },

    /// Manage audio input devices
    Device {
        #[command(subcommand)]
        action: DeviceAction,
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

    /// Record and transcribe audio (system audio or microphone)
    Record {
        /// Audio source: mic, monitor (system audio), or both
        #[arg(short, long, default_value = "mic")]
        source: String,

        /// Output file (e.g., meeting.txt, call.srt)
        #[arg(short, long)]
        output: Option<String>,

        /// Enable speaker diarization
        #[arg(short, long)]
        diarize: bool,

        /// Live mode: print transcription as it happens
        #[arg(short, long)]
        live: bool,

        /// Output format: text, timestamped, srt, vtt
        #[arg(short = 'F', long, default_value = "text")]
        format: String,
    },

    /// Control recording on a running daemon
    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    Recording {
        #[command(subcommand)]
        action: RecordingAction,
    },

    /// Manage autostart service
    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    Service {
        #[command(subcommand)]
        action: ServiceAction,
    },

    /// Manage secrets in system keyring
    Secret {
        #[command(subcommand)]
        action: SecretAction,
    },

    /// Manage API keys for REST API authentication
    ApiKey {
        #[command(subcommand)]
        action: ApiKeyAction,
    },

    /// Summarize a transcription or audio file using LLM
    Summarize {
        /// Input file (transcription text or audio file)
        input: String,

        /// Template to use (standup, meeting, retro, 1on1, summary)
        #[arg(short, long, default_value = "meeting")]
        template: String,

        /// LLM provider (ollama, openai)
        #[arg(short, long)]
        provider: Option<String>,

        /// Model name (overrides config)
        #[arg(short, long)]
        model: Option<String>,

        /// Output file (default: stdout)
        #[arg(short, long)]
        output: Option<String>,

        /// Output format (markdown, json)
        #[arg(short = 'f', long, default_value = "markdown")]
        format: String,

        /// List available templates
        #[arg(long)]
        list_templates: bool,
    },
}

/// Recording control actions (sent to daemon via D-Bus on Linux, IPC on macOS/Windows)
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
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

/// Service management actions
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
#[derive(Subcommand)]
enum ServiceAction {
    /// Install autostart service (run on login)
    Install,

    /// Remove autostart service
    Uninstall,

    /// Show service status
    Status,
}

/// Secret management actions
#[derive(Subcommand)]
enum SecretAction {
    /// Store a secret in the system keyring
    Set {
        /// Name of the secret (e.g., "ollama-api", "webhook-url")
        name: String,
    },

    /// List information about secret storage
    List,

    /// Delete a secret from the keyring
    Delete {
        /// Name of the secret to delete
        name: String,
    },

    /// Show a secret value (use with caution)
    Show {
        /// Name of the secret to display
        name: String,

        /// Skip confirmation prompt
        #[arg(short, long)]
        force: bool,
    },

    /// Check if keyring is available on this system
    Check,
}

/// API key management actions
#[derive(Subcommand)]
enum ApiKeyAction {
    /// Generate a new random API key
    Generate,

    /// Set an API key (hashes and saves to config)
    Set {
        /// The API key to set
        key: String,
    },

    /// Show if API key is configured
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

    /// Load model into GPU memory (requires running daemon)
    Load,

    /// Unload model from GPU memory (requires running daemon)
    Unload,
}

#[derive(Subcommand)]
enum DeviceAction {
    /// List available audio input devices
    List {
        /// Output format (text, json)
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Set the input device
    Set {
        /// Device ID (use `device list` to see available devices)
        id: String,
    },

    /// Set channel selection for a device
    Channels {
        /// Channel selection: "all" or comma-separated indices (e.g., "0,1")
        selection: String,
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

fn main() -> anyhow::Result<()> {
    // Install panic handler first, before anything else
    panic_handler::install();

    let cli = Cli::parse();

    // Handle daemonization BEFORE starting tokio runtime
    // Fork + threads = broken, so we must fork first
    #[cfg(unix)]
    if let Commands::Start {
        foreground: false,
        no_tray,
    } = &cli.command
    {
        // Daemonize before any async runtime or threads are started
        daemon::daemonize_early(!no_tray)?;
    }

    // Now start the tokio runtime (after fork if daemonizing)
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(async_main(cli))
}

async fn async_main(cli: Cli) -> anyhow::Result<()> {
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
            // Check for first run and launch setup wizard if needed
            #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
            if gui::is_first_run() {
                info!("First run detected, launching setup wizard...");
                gui::run_wizard()?;
            }

            info!("Starting OpenHush daemon...");
            // Note: daemonization already happened in main() before tokio started
            daemon::run(foreground, !no_tray).await?;
        }

        Commands::Preferences => {
            info!("Opening preferences...");
            gui::run_preferences()?;
        }

        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        Commands::Setup => {
            info!("Running setup wizard...");
            gui::run_wizard()?;
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
            translation,
            translation_engine,
            translation_target,
            show,
        } => {
            if show {
                config::show()?;
            } else {
                config::update(
                    hotkey,
                    model,
                    language,
                    translate,
                    llm,
                    translation,
                    translation_engine,
                    translation_target,
                )?;
            }
        }

        Commands::Model { action } => match action {
            ModelAction::Download { name } => {
                use engine::whisper::{
                    download_model, format_size, model_size_bytes, WhisperModel,
                };
                use std::io::Write;

                // Handle wake-word model download separately
                if name == "wake-word" {
                    use input::wake_word::WakeWordDetector;

                    if WakeWordDetector::models_available() {
                        println!("Wake word models already downloaded.");
                        return Ok(());
                    }

                    println!("Downloading wake word models (~3.7 MB)...");
                    WakeWordDetector::download_models().await.map_err(|e| {
                        anyhow::anyhow!("Failed to download wake word models: {}", e)
                    })?;
                    println!("Wake word models downloaded successfully.");
                    return Ok(());
                }

                // Handle M2M-100 translation model downloads
                if name.starts_with("m2m100") || name == "m2m-100" {
                    use translation::{download_m2m100_model, is_m2m100_downloaded, M2M100Model};

                    let model: M2M100Model = name.parse().map_err(|e: String| {
                        anyhow::anyhow!(
                            "{}\nAvailable: m2m100-418m (1.5GB), m2m100-1.2b (4.5GB)",
                            e
                        )
                    })?;

                    if is_m2m100_downloaded(model) {
                        println!("M2M-100 {} already downloaded.", model.name());
                        return Ok(());
                    }

                    println!(
                        "Downloading M2M-100 {} (~{} MB)...\n\
                         Note: ONNX models need to be exported from HuggingFace.",
                        model.name(),
                        model.vram_mb()
                    );

                    let mut current_file = String::new();
                    let mut last_percent = 0u32;

                    let path = download_m2m100_model(model, |filename, downloaded, total| {
                        if current_file != filename {
                            if !current_file.is_empty() {
                                println!();
                            }
                            current_file = filename.to_string();
                            last_percent = 0;
                            print!("  {}: ", filename);
                            let _ = std::io::stdout().flush();
                        }

                        if total > 0 {
                            let percent = ((downloaded as f64 / total as f64) * 100.0) as u32;
                            if percent > last_percent {
                                last_percent = percent;
                                print!("\r  {}: {}%", filename, percent);
                                let _ = std::io::stdout().flush();
                            }
                        }
                    })
                    .await
                    .map_err(|e| anyhow::anyhow!("{}", e))?;

                    println!(
                        "\n\nM2M-100 {} downloaded to: {}",
                        model.name(),
                        path.display()
                    );
                    println!("\nTo use M2M-100 for translation:");
                    println!("  openhush config --translation true --translation-engine m2m100 --translation-target de");
                    return Ok(());
                }

                let model: WhisperModel = name.parse().map_err(|()| {
                    anyhow::anyhow!(
                        "Unknown model '{}'. Available: tiny, base, small, medium, large-v3, wake-word, m2m100-418m, m2m100-1.2b",
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

                // Show wake-word model status
                println!("\nWake word models:\n");
                println!(
                    "  {:<12} {:<10} {:<10} Description",
                    "Model", "Size", "Status"
                );
                println!("  {}", "-".repeat(60));
                {
                    use input::wake_word::WakeWordDetector;
                    let status = if WakeWordDetector::models_available() {
                        "✓ local"
                    } else {
                        "remote"
                    };
                    let desc = "\"Hey Jarvis\" wake word detection";
                    println!(
                        "  {:<12} {:<10} {:<10} {}",
                        "wake-word", "~3.7 MB", status, desc
                    );
                }

                // Show M2M-100 translation models
                println!("\nTranslation models (M2M-100):\n");
                println!(
                    "  {:<12} {:<10} {:<10} Description",
                    "Model", "VRAM", "Status"
                );
                println!("  {}", "-".repeat(60));
                {
                    use translation::{is_m2m100_downloaded, M2M100Model};

                    for (model, desc) in [
                        (M2M100Model::Small, "418M params, balanced"),
                        (M2M100Model::Large, "1.2B params, best quality"),
                    ] {
                        let status = if is_m2m100_downloaded(model) {
                            "✓ local"
                        } else {
                            "remote"
                        };
                        let vram = format!("~{} MB", model.vram_mb());
                        println!(
                            "  {:<12} {:<10} {:<10} {}",
                            model.name(),
                            vram,
                            status,
                            desc
                        );
                    }
                }

                println!("\nUse 'openhush model download <name>' to download a model.");
            }
            ModelAction::Remove { name } => {
                use engine::whisper::{remove_model, WhisperModel};

                // Handle wake-word model removal separately
                if name == "wake-word" {
                    use input::wake_word::WakeWordDetector;

                    if !WakeWordDetector::models_available() {
                        println!("Wake word models are not installed.");
                        return Ok(());
                    }

                    WakeWordDetector::remove_models()
                        .map_err(|e| anyhow::anyhow!("Failed to remove wake word models: {}", e))?;
                    println!("Removed wake word models.");
                    return Ok(());
                }

                // Handle M2M-100 translation model removal
                if name.starts_with("m2m100") || name == "m2m-100" {
                    use translation::{is_m2m100_downloaded, remove_m2m100_model, M2M100Model};

                    let model: M2M100Model = name.parse().map_err(|e: String| {
                        anyhow::anyhow!("{}\nAvailable: m2m100-418m, m2m100-1.2b", e)
                    })?;

                    if !is_m2m100_downloaded(model) {
                        println!("M2M-100 {} is not installed.", model.name());
                        return Ok(());
                    }

                    remove_m2m100_model(model)
                        .map_err(|e| anyhow::anyhow!("Failed to remove M2M-100 model: {}", e))?;
                    println!("Removed M2M-100 {} model.", model.name());
                    return Ok(());
                }

                let model: WhisperModel = name.parse().map_err(|()| {
                    anyhow::anyhow!(
                        "Unknown model '{}'. Available: tiny, base, small, medium, large-v3, wake-word, m2m100-418m, m2m100-1.2b",
                        name
                    )
                })?;

                remove_model(model)?;
                println!("Removed model: {}", model.filename());
            }
            ModelAction::Load => {
                #[cfg(target_os = "linux")]
                {
                    use dbus::DbusClient;

                    let client = match DbusClient::connect().await {
                        Ok(c) => c,
                        Err(e) => {
                            eprintln!("Failed to connect to D-Bus: {}", e);
                            std::process::exit(1);
                        }
                    };

                    if !client.is_daemon_running().await {
                        eprintln!("Daemon is not running. Start it with: openhush start");
                        std::process::exit(1);
                    }

                    match client.load_model().await {
                        Ok(()) => println!("Model loaded successfully"),
                        Err(e) => {
                            eprintln!("Failed to load model: {}", e);
                            std::process::exit(1);
                        }
                    }
                }

                #[cfg(any(target_os = "macos", target_os = "windows"))]
                {
                    use crate::ipc::{IpcClient, IpcCommand};

                    let mut client = match IpcClient::connect() {
                        Ok(c) => c,
                        Err(e) => {
                            eprintln!("Failed to connect to daemon: {}", e);
                            std::process::exit(1);
                        }
                    };

                    match client.send(IpcCommand::LoadModel) {
                        Ok(response) => {
                            if response.ok {
                                println!("Model loaded successfully");
                            } else if let Some(err) = response.error {
                                eprintln!("Failed to load model: {}", err);
                                std::process::exit(1);
                            }
                        }
                        Err(e) => {
                            eprintln!("Failed to load model: {}", e);
                            std::process::exit(1);
                        }
                    }
                }
            }
            ModelAction::Unload => {
                #[cfg(target_os = "linux")]
                {
                    use dbus::DbusClient;

                    let client = match DbusClient::connect().await {
                        Ok(c) => c,
                        Err(e) => {
                            eprintln!("Failed to connect to D-Bus: {}", e);
                            std::process::exit(1);
                        }
                    };

                    if !client.is_daemon_running().await {
                        eprintln!("Daemon is not running. Start it with: openhush start");
                        std::process::exit(1);
                    }

                    match client.unload_model().await {
                        Ok(()) => println!("Model unloaded successfully"),
                        Err(e) => {
                            eprintln!("Failed to unload model: {}", e);
                            std::process::exit(1);
                        }
                    }
                }

                #[cfg(any(target_os = "macos", target_os = "windows"))]
                {
                    use crate::ipc::{IpcClient, IpcCommand};

                    let mut client = match IpcClient::connect() {
                        Ok(c) => c,
                        Err(e) => {
                            eprintln!("Failed to connect to daemon: {}", e);
                            std::process::exit(1);
                        }
                    };

                    match client.send(IpcCommand::UnloadModel) {
                        Ok(response) => {
                            if response.ok {
                                println!("Model unloaded successfully");
                            } else if let Some(err) = response.error {
                                eprintln!("Failed to unload model: {}", err);
                                std::process::exit(1);
                            }
                        }
                        Err(e) => {
                            eprintln!("Failed to unload model: {}", e);
                            std::process::exit(1);
                        }
                    }
                }
            }
        },

        Commands::Device { action } => match action {
            DeviceAction::List { format } => {
                let devices = input::enumerate_audio_inputs();

                if format == "json" {
                    let json = serde_json::to_string_pretty(&devices)
                        .map_err(|e| anyhow::anyhow!("JSON error: {}", e))?;
                    println!("{}", json);
                } else {
                    println!("Audio input devices:\n");
                    println!("  {:<40} {:<12} {:<10} Default", "Name", "Type", "Channels");
                    println!("  {}", "-".repeat(75));

                    for device in &devices {
                        let type_str = match device.device_type {
                            input::AudioDeviceType::Microphone => "mic",
                            input::AudioDeviceType::Monitor => "monitor",
                        };
                        let default_str = if device.is_default { "✓" } else { "" };
                        let channels = device.channel_names.join(", ");
                        println!(
                            "  {:<40} {:<12} {:<10} {}",
                            if device.name.len() > 38 {
                                format!("{}...", &device.name[..35])
                            } else {
                                device.name.clone()
                            },
                            type_str,
                            format!("{} ({})", device.channel_count, channels),
                            default_str
                        );
                    }

                    println!("\n  Device IDs (for config):");
                    for device in &devices {
                        println!("    {}", device.id);
                    }

                    println!("\n  To set input device:");
                    println!("    openhush device set <device_id>");
                }
            }
            DeviceAction::Set { id } => {
                let mut config = config::Config::load().unwrap_or_default();

                // Verify device exists
                let devices = input::enumerate_audio_inputs();
                if !devices.iter().any(|d| d.id == id) {
                    eprintln!("Device not found: {}", id);
                    eprintln!("Use `openhush device list` to see available devices.");
                    std::process::exit(1);
                }

                config.audio.input_device = Some(id.clone());
                config.save()?;
                println!("Input device set to: {}", id);
            }
            DeviceAction::Channels { selection } => {
                let channels = config::ChannelSelection::from_cli_arg(&selection)
                    .map_err(|e| anyhow::anyhow!("{}", e))?;

                let mut config = config::Config::load().unwrap_or_default();
                config.audio.channels = channels.clone();
                config.save()?;

                match channels {
                    config::ChannelSelection::All => {
                        println!("Channel selection set to: all");
                    }
                    config::ChannelSelection::Select(indices) => {
                        println!("Channel selection set to: {:?}", indices);
                    }
                }
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

        Commands::Record {
            source,
            output,
            diarize,
            live,
            format,
        } => {
            use crate::recording::{RecordingConfig, RecordingSession};

            let audio_source = source.parse().map_err(|e: String| anyhow::anyhow!(e))?;

            let config = RecordingConfig {
                source: audio_source,
                output_file: output,
                enable_diarization: diarize,
                live_mode: live,
                output_format: format.parse().unwrap_or_default(),
            };

            info!("Starting recording session...");
            let session = RecordingSession::new(config)?;
            session.run().await?;
        }

        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        Commands::Recording { action } => {
            #[cfg(target_os = "linux")]
            {
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

            #[cfg(any(target_os = "macos", target_os = "windows"))]
            {
                use ipc::{IpcClient, IpcCommand};

                let mut client = match IpcClient::connect() {
                    Ok(c) => c,
                    Err(e) => {
                        eprintln!("Failed to connect to daemon: {}", e);
                        eprintln!("Is the daemon running? Try: openhush start");
                        std::process::exit(1);
                    }
                };

                match action {
                    RecordingAction::Start => match client.send(IpcCommand::StartRecording) {
                        Ok(response) => {
                            if response.ok {
                                println!("Recording started");
                            } else if let Some(err) = response.error {
                                eprintln!("Failed to start recording: {}", err);
                                std::process::exit(1);
                            }
                        }
                        Err(e) => {
                            eprintln!("Failed to start recording: {}", e);
                            std::process::exit(1);
                        }
                    },
                    RecordingAction::Stop => match client.send(IpcCommand::StopRecording) {
                        Ok(response) => {
                            if response.ok {
                                println!("Recording stopped");
                            } else if let Some(err) = response.error {
                                eprintln!("Failed to stop recording: {}", err);
                                std::process::exit(1);
                            }
                        }
                        Err(e) => {
                            eprintln!("Failed to stop recording: {}", e);
                            std::process::exit(1);
                        }
                    },
                    RecordingAction::Toggle => match client.send(IpcCommand::ToggleRecording) {
                        Ok(response) => {
                            if response.ok {
                                println!("Recording toggled");
                            } else if let Some(err) = response.error {
                                eprintln!("Failed to toggle recording: {}", err);
                                std::process::exit(1);
                            }
                        }
                        Err(e) => {
                            eprintln!("Failed to toggle recording: {}", e);
                            std::process::exit(1);
                        }
                    },
                    RecordingAction::Status => match client.send(IpcCommand::Status) {
                        Ok(response) => {
                            if response.ok {
                                let recording = response.recording.unwrap_or(false);
                                let model = response.model_loaded.unwrap_or(false);
                                let version = response.version.unwrap_or_default();
                                println!(
                                    "Status: {}",
                                    if recording { "recording" } else { "idle" }
                                );
                                println!("Model loaded: {}", model);
                                println!("Version: {}", version);
                            } else if let Some(err) = response.error {
                                eprintln!("Failed to get status: {}", err);
                                std::process::exit(1);
                            }
                        }
                        Err(e) => {
                            eprintln!("Failed to get status: {}", e);
                            std::process::exit(1);
                        }
                    },
                }
            }
        }

        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        Commands::Service { action } => match action {
            ServiceAction::Install => {
                service::install()?;
            }
            ServiceAction::Uninstall => {
                service::uninstall()?;
            }
            ServiceAction::Status => {
                let status = service::status()?;
                print!("{}", status);
            }
        },

        Commands::Secret { action } => match action {
            SecretAction::Set { name } => {
                secrets::cli::handle_set(&name)?;
            }
            SecretAction::List => {
                secrets::cli::handle_list();
            }
            SecretAction::Delete { name } => {
                secrets::cli::handle_delete(&name)?;
            }
            SecretAction::Show { name, force } => {
                secrets::cli::handle_show(&name, force)?;
            }
            SecretAction::Check => {
                secrets::cli::handle_check();
            }
        },

        Commands::ApiKey { action } => match action {
            ApiKeyAction::Generate => {
                let key = api::generate_api_key();
                let hash = api::hash_api_key(&key);

                println!("Generated API key:\n");
                println!("  {}\n", key);
                println!("Save this key securely - it cannot be recovered!");
                println!("\nTo configure, add to ~/.config/openhush/config.toml:\n");
                println!("[api]");
                println!("enabled = true");
                println!("api_key_hash = \"{}\"", hash);
            }
            ApiKeyAction::Set { key } => {
                let hash = api::hash_api_key(&key);
                let mut config = config::Config::load()?;
                config.api.api_key_hash = Some(hash.clone());
                config.save()?;
                println!("API key hash saved to config.");
                println!("\nTo enable the API, also set:");
                println!("[api]");
                println!("enabled = true");
            }
            ApiKeyAction::Status => {
                let config = config::Config::load()?;
                if config.api.enabled {
                    println!("REST API: enabled");
                    println!("Bind address: {}", config.api.bind);
                    if config.api.api_key_hash.is_some() {
                        println!("API key: configured");
                    } else {
                        println!("API key: NOT configured (API is open!)");
                    }
                    println!(
                        "Swagger UI: {}",
                        if config.api.swagger_ui {
                            "enabled"
                        } else {
                            "disabled"
                        }
                    );
                } else {
                    println!("REST API: disabled");
                    println!("\nTo enable, add to config.toml:");
                    println!("[api]");
                    println!("enabled = true");
                }
            }
        },

        Commands::Summarize {
            input,
            template,
            provider,
            model,
            output,
            format,
            list_templates,
        } => {
            if list_templates {
                summarization::list_templates();
                return Ok(());
            }

            let config = config::Config::load()?;

            // Read input: text file or audio file
            let transcript = if input.ends_with(".txt") || input.ends_with(".md") {
                std::fs::read_to_string(&input)
                    .map_err(|e| anyhow::anyhow!("Failed to read {}: {}", input, e))?
            } else if input.ends_with(".wav")
                || input.ends_with(".mp3")
                || input.ends_with(".m4a")
                || input.ends_with(".flac")
            {
                // Transcribe audio file first
                use std::path::Path;

                eprintln!("Transcribing audio file...");
                let file_path = Path::new(&input);
                let audio = input::load_wav_file(file_path, config.audio.resampling_quality)?;

                // Initialize Whisper engine
                let data_dir = config::Config::data_dir()?;
                let model_name = config.transcription.effective_model();
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

                let engine = engine::whisper::WhisperEngine::new(
                    &model_path,
                    &config.transcription.language,
                    config.transcription.translate,
                    config.transcription.device.to_lowercase() != "cpu",
                )?;
                let result = engine.transcribe(&audio)?;
                result.text
            } else {
                // Assume it's raw text from stdin or a text file
                std::fs::read_to_string(&input)
                    .map_err(|e| anyhow::anyhow!("Failed to read {}: {}", input, e))?
            };

            // Determine provider
            let provider_name = provider.unwrap_or(config.summarization.default_provider.clone());

            // Create LLM provider
            let llm_provider: Box<dyn summarization::LlmProvider> =
                match provider_name.as_str() {
                    "ollama" => {
                        let mut ollama_config = summarization::OllamaConfig {
                            url: config.summarization.ollama.url.clone(),
                            model: config.summarization.ollama.model.clone(),
                            timeout_secs: config.summarization.ollama.timeout_secs,
                        };
                        if let Some(m) = model.as_ref() {
                            ollama_config.model = m.clone();
                        }
                        Box::new(summarization::OllamaProvider::new(ollama_config))
                    }
                    "openai" => {
                        let store = secrets::SecretStore::new();
                        let api_key =
                            secrets::resolve_secret(&config.summarization.openai.api_key, &store)
                                .map_err(|e| anyhow::anyhow!("Failed to resolve API key: {}", e))?;

                        let mut openai_config = summarization::OpenAiConfig {
                            api_key,
                            model: config.summarization.openai.model.clone(),
                            base_url: config.summarization.openai.base_url.clone(),
                            timeout_secs: config.summarization.openai.timeout_secs,
                        };
                        if let Some(m) = model.as_ref() {
                            openai_config.model = m.clone();
                        }
                        Box::new(summarization::OpenAiProvider::new(openai_config).map_err(
                            |e| anyhow::anyhow!("Failed to create OpenAI provider: {}", e),
                        )?)
                    }
                    _ => {
                        anyhow::bail!(
                            "Unknown provider '{}'. Use 'ollama' or 'openai'.",
                            provider_name
                        );
                    }
                };

            // Check provider availability
            let rt = tokio::runtime::Runtime::new()?;
            if !rt.block_on(llm_provider.is_available()) {
                eprintln!(
                    "Warning: {} provider may not be available. Attempting anyway...",
                    provider_name
                );
            }

            // Create summarizer
            let summarizer = summarization::Summarizer::new(llm_provider);

            // Create context
            let ctx = summarization::TemplateContext::new(
                transcript,
                chrono::Utc::now().format("%Y-%m-%d").to_string(),
                "unknown".to_string(), // TODO: extract from audio metadata
            );

            // Use configured or specified template
            let template_name = if template == "meeting" {
                config.summarization.default_template.clone()
            } else {
                template
            };

            // Run summarization
            eprintln!("Summarizing with template '{}'...", template_name);
            let result = rt
                .block_on(summarizer.summarize(&template_name, &ctx))
                .map_err(|e| anyhow::anyhow!("Summarization failed: {}", e))?;

            // Output result
            let output_text = match format.as_str() {
                "json" => {
                    let json = serde_json::json!({
                        "summary": result.summary,
                        "template": result.template_used,
                        "provider": result.provider_used,
                        "model": result.model_used,
                        "tokens_used": result.tokens_used,
                    });
                    serde_json::to_string_pretty(&json)?
                }
                _ => result.summary,
            };

            if let Some(output_path) = output {
                std::fs::write(&output_path, &output_text)
                    .map_err(|e| anyhow::anyhow!("Failed to write {}: {}", output_path, e))?;
                eprintln!("Summary written to {}", output_path);
            } else {
                println!("{}", output_text);
            }
        }
    }

    Ok(())
}
