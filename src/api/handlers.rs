//! API request handlers.

use axum::{extract::State, http::StatusCode, Json};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::state::{ApiCommand, ApiState};

/// Health check response.
#[derive(Debug, Serialize, ToSchema)]
pub struct HealthResponse {
    /// Always "ok" if server is responding
    pub status: String,
    /// API version
    pub version: String,
}

/// Daemon status response.
#[derive(Debug, Serialize, ToSchema)]
pub struct StatusResponse {
    /// Whether daemon is running
    pub running: bool,
    /// Whether currently recording
    pub recording: bool,
    /// Number of pending transcription jobs in queue
    pub queue_depth: u32,
    /// Current Whisper model name
    pub model: String,
    /// OpenHush version
    pub version: String,
}

/// Generic success response.
#[derive(Debug, Serialize, ToSchema)]
pub struct SuccessResponse {
    /// Whether the operation succeeded
    pub ok: bool,
    /// Optional message
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Error response.
#[derive(Debug, Serialize, ToSchema)]
pub struct ErrorResponse {
    /// Always false for errors
    pub ok: bool,
    /// Error message
    pub error: String,
}

/// Recording action request.
#[derive(Debug, Deserialize, ToSchema)]
#[allow(dead_code)] // Schema-only type for OpenAPI documentation
pub struct RecordingAction {
    /// Action: "start", "stop", or "toggle"
    pub action: String,
}

/// Health check endpoint (no auth required).
///
/// Returns basic health status for load balancers and monitoring.
#[utoipa::path(
    get,
    path = "/api/v1/health",
    responses(
        (status = 200, description = "Server is healthy", body = HealthResponse)
    ),
    tag = "Health"
)]
pub async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

/// Get daemon status.
///
/// Returns current recording state, queue depth, and model info.
#[utoipa::path(
    get,
    path = "/api/v1/status",
    responses(
        (status = 200, description = "Current daemon status", body = StatusResponse),
        (status = 401, description = "Unauthorized - missing or invalid API key")
    ),
    security(
        ("api_key" = [])
    ),
    tag = "Status"
)]
pub async fn get_status(State(state): State<ApiState>) -> Json<StatusResponse> {
    let status = state.status.read().await;
    Json(StatusResponse {
        running: status.running,
        recording: status.recording,
        queue_depth: status.queue_depth,
        model: status.model.clone(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

/// Start recording.
///
/// Begins audio capture for transcription.
#[utoipa::path(
    post,
    path = "/api/v1/recording/start",
    responses(
        (status = 200, description = "Recording started", body = SuccessResponse),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Failed to start recording", body = ErrorResponse)
    ),
    security(
        ("api_key" = [])
    ),
    tag = "Recording"
)]
pub async fn start_recording(
    State(state): State<ApiState>,
) -> Result<Json<SuccessResponse>, (StatusCode, Json<ErrorResponse>)> {
    state
        .cmd_tx
        .send(ApiCommand::StartRecording)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    ok: false,
                    error: format!("Failed to send command: {}", e),
                }),
            )
        })?;

    Ok(Json(SuccessResponse {
        ok: true,
        message: Some("Recording started".to_string()),
    }))
}

/// Stop recording.
///
/// Stops audio capture and triggers transcription.
#[utoipa::path(
    post,
    path = "/api/v1/recording/stop",
    responses(
        (status = 200, description = "Recording stopped", body = SuccessResponse),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Failed to stop recording", body = ErrorResponse)
    ),
    security(
        ("api_key" = [])
    ),
    tag = "Recording"
)]
pub async fn stop_recording(
    State(state): State<ApiState>,
) -> Result<Json<SuccessResponse>, (StatusCode, Json<ErrorResponse>)> {
    state
        .cmd_tx
        .send(ApiCommand::StopRecording)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    ok: false,
                    error: format!("Failed to send command: {}", e),
                }),
            )
        })?;

    Ok(Json(SuccessResponse {
        ok: true,
        message: Some("Recording stopped".to_string()),
    }))
}

/// Toggle recording.
///
/// Starts recording if stopped, stops if recording.
#[utoipa::path(
    post,
    path = "/api/v1/recording/toggle",
    responses(
        (status = 200, description = "Recording toggled", body = SuccessResponse),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Failed to toggle recording", body = ErrorResponse)
    ),
    security(
        ("api_key" = [])
    ),
    tag = "Recording"
)]
pub async fn toggle_recording(
    State(state): State<ApiState>,
) -> Result<Json<SuccessResponse>, (StatusCode, Json<ErrorResponse>)> {
    state
        .cmd_tx
        .send(ApiCommand::ToggleRecording)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    ok: false,
                    error: format!("Failed to send command: {}", e),
                }),
            )
        })?;

    Ok(Json(SuccessResponse {
        ok: true,
        message: Some("Recording toggled".to_string()),
    }))
}
