//! API router setup with Swagger UI and middleware.

use axum::{
    middleware,
    routing::{get, post},
    Router,
};
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use super::auth::require_api_key;
use super::handlers::{
    self, get_status, health, start_recording, stop_recording, toggle_recording, ErrorResponse,
    HealthResponse, RecordingAction, StatusResponse, SuccessResponse,
};
use super::state::ApiState;
use crate::config::ApiConfig;

/// OpenAPI documentation.
#[derive(OpenApi)]
#[openapi(
    info(
        title = "OpenHush API",
        version = "1.0.0",
        description = "REST API for controlling the OpenHush voice-to-text daemon",
        license(name = "MIT", url = "https://opensource.org/licenses/MIT"),
        contact(name = "OpenHush", url = "https://github.com/claymore666/openhush")
    ),
    paths(
        handlers::health,
        handlers::get_status,
        handlers::start_recording,
        handlers::stop_recording,
        handlers::toggle_recording,
    ),
    components(
        schemas(
            HealthResponse,
            StatusResponse,
            SuccessResponse,
            ErrorResponse,
            RecordingAction,
        )
    ),
    tags(
        (name = "Health", description = "Health check endpoints"),
        (name = "Status", description = "Daemon status endpoints"),
        (name = "Recording", description = "Recording control endpoints"),
    ),
    modifiers(&SecurityAddon)
)]
struct ApiDoc;

/// Add API key security scheme to OpenAPI spec.
struct SecurityAddon;

impl utoipa::Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "api_key",
                utoipa::openapi::security::SecurityScheme::ApiKey(
                    utoipa::openapi::security::ApiKey::Header(
                        utoipa::openapi::security::ApiKeyValue::new("X-API-Key"),
                    ),
                ),
            );
        }
    }
}

/// Create the API router with all routes and middleware.
pub fn create_router(state: ApiState, config: &ApiConfig) -> Router {
    // Public routes (no auth)
    let public_routes = Router::new().route("/api/v1/health", get(health));

    // Protected routes (require API key)
    let protected_routes = Router::new()
        .route("/api/v1/status", get(get_status))
        .route("/api/v1/recording/start", post(start_recording))
        .route("/api/v1/recording/stop", post(stop_recording))
        .route("/api/v1/recording/toggle", post(toggle_recording))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            require_api_key,
        ));

    // Build main router
    let mut router = Router::new()
        .merge(public_routes)
        .merge(protected_routes)
        .with_state(state);

    // Add Swagger UI if enabled
    if config.swagger_ui {
        router = router
            .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()));
    }

    // Add CORS if origins configured
    let cors = if config.cors_origins.is_empty() {
        // No CORS (same-origin only)
        CorsLayer::new()
    } else if config.cors_origins.iter().any(|o| o == "*") {
        // Allow all origins
        CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any)
    } else {
        // Allow specific origins
        let origins: Vec<_> = config
            .cors_origins
            .iter()
            .filter_map(|o| o.parse().ok())
            .collect();
        CorsLayer::new()
            .allow_origin(origins)
            .allow_methods(Any)
            .allow_headers(Any)
    };

    router.layer(cors).layer(TraceLayer::new_for_http())
}
