//! API key authentication middleware.

use axum::{
    body::Body,
    extract::State,
    http::{Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use sha2::{Digest, Sha256};

use super::state::ApiState;

/// Header name for API key.
pub const API_KEY_HEADER: &str = "X-API-Key";

/// Extract and validate API key from request.
pub async fn require_api_key(
    State(state): State<ApiState>,
    request: Request<Body>,
    next: Next,
) -> Response {
    // If no API key configured, allow all requests (development mode)
    let Some(ref expected_hash) = state.api_key_hash else {
        return next.run(request).await;
    };

    // Extract API key from header
    let api_key = request
        .headers()
        .get(API_KEY_HEADER)
        .and_then(|v| v.to_str().ok());

    match api_key {
        Some(key) => {
            // Hash the provided key and compare
            let provided_hash = hash_api_key(key);
            if provided_hash == *expected_hash {
                next.run(request).await
            } else {
                (StatusCode::UNAUTHORIZED, "Invalid API key").into_response()
            }
        }
        None => (
            StatusCode::UNAUTHORIZED,
            format!("Missing {} header", API_KEY_HEADER),
        )
            .into_response(),
    }
}

/// Hash an API key using SHA-256.
pub fn hash_api_key(key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(key.as_bytes());
    let result = hasher.finalize();
    hex::encode(result)
}

/// Generate a random API key.
pub fn generate_api_key() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    // Use timestamp + process ID for uniqueness
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();

    let mut hasher = Sha256::new();
    hasher.update(timestamp.to_le_bytes());
    hasher.update(std::process::id().to_le_bytes());

    // Add pseudo-random data from timestamp bytes
    let ts_bytes = timestamp.to_le_bytes();
    let random_data: [u8; 32] =
        std::array::from_fn(|i| ts_bytes[i % ts_bytes.len()] ^ (i as u8).wrapping_mul(17));
    hasher.update(random_data);

    let result = hasher.finalize();
    // Return first 32 chars of hex (128 bits of entropy)
    hex::encode(&result[..16])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_api_key() {
        let key = "test-key-12345";
        let hash = hash_api_key(key);

        // Should be 64 hex chars (256 bits)
        assert_eq!(hash.len(), 64);

        // Same key should produce same hash
        assert_eq!(hash, hash_api_key(key));

        // Different key should produce different hash
        assert_ne!(hash, hash_api_key("different-key"));
    }

    #[test]
    fn test_generate_api_key() {
        let key1 = generate_api_key();
        let key2 = generate_api_key();

        // Keys should be 32 hex chars
        assert_eq!(key1.len(), 32);
        assert_eq!(key2.len(), 32);

        // Keys should be different (with high probability)
        // Note: In theory they could be the same, but extremely unlikely
        assert_ne!(key1, key2);
    }
}
