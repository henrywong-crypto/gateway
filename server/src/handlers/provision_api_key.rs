use apikeys::{create_api_key, get_active_api_key};
use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use jsonwebtoken::{decode, decode_header};
use jwks::{Jwks, jwk_to_decoding_key};
use myhandlers::AppState;
use serde::{Deserialize, Serialize};
use tracing::{error, info};
use validation::ValidationBuilder;

#[derive(Serialize)]
struct ApiKeyResponse {
    api_key: String,
}

#[derive(Deserialize)]
struct CognitoClaims {
    email: Option<String>,
}

/// POST /api/v1/api-keys
///
/// Accepts `Authorization: Bearer <cognito_access_token>`.
/// Validates the JWT against gateway Cognito JWKS, extracts the user email,
/// creates the user if needed, and returns an existing active API key or
/// creates a new one.
pub async fn provision_api_key(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Response, Response> {
    let token = extract_bearer_token(&headers).ok_or_else(|| {
        (StatusCode::UNAUTHORIZED, "Missing or invalid Authorization header").into_response()
    })?;

    let email = validate_jwt_and_extract_email(&token, &state).await.map_err(|e| {
        error!("JWT validation failed: {e}");
        (StatusCode::UNAUTHORIZED, "Invalid or expired token").into_response()
    })?;

    info!("provisioning API key for {email}");

    // Create user if not exists (ignore unique constraint violation)
    if let Err(e) = users::create_user(&state.db_pool, &email).await {
        let err_str = e.to_string();
        if !err_str.contains("duplicate key") && !err_str.contains("unique constraint") {
            error!("create_user failed: {e}");
            return Err((StatusCode::INTERNAL_SERVER_ERROR, "Internal error").into_response());
        }
    }

    // Return existing active key if available
    match get_active_api_key(&state.db_pool, &email).await {
        Ok(Some(key)) => {
            return Ok(Json(ApiKeyResponse { api_key: key }).into_response());
        }
        Ok(None) => {}
        Err(e) => {
            error!("get_active_api_key failed: {e}");
            return Err(
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal error").into_response()
            );
        }
    }

    // Create new key
    let api_key = create_api_key(&state.db_pool, &email).await.map_err(|e| {
        error!("create_api_key failed: {e}");
        (StatusCode::INTERNAL_SERVER_ERROR, "Internal error").into_response()
    })?;

    Ok(Json(ApiKeyResponse {
        api_key: api_key.to_string(),
    })
    .into_response())
}

fn extract_bearer_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|v| v.trim().to_string())
}

async fn validate_jwt_and_extract_email(
    token: &str,
    state: &AppState,
) -> anyhow::Result<String> {
    // Decode header to get kid
    let header = decode_header(token)?;
    let kid = header.kid.ok_or_else(|| anyhow::anyhow!("JWT missing kid"))?;

    // Fetch JWKS
    let jwks = Jwks::builder()
        .region(&state.cognito_region)
        .user_pool_id(&state.cognito_user_pool_id)
        .build()
        .await?;

    let jwk = jwks
        .find_jwk(&kid)
        .ok_or_else(|| anyhow::anyhow!("No matching key found in JWKS"))?;

    let decoding_key = jwk_to_decoding_key(&jwk)?;

    // Cognito access tokens don't have aud claim, so skip client_id
    let validation = ValidationBuilder::new()
        .region(&state.cognito_region)
        .user_pool_id(&state.cognito_user_pool_id)
        .build()?;

    let token_data = decode::<CognitoClaims>(token, &decoding_key, &validation)?;

    token_data
        .claims
        .email
        .filter(|e| !e.is_empty())
        .ok_or_else(|| anyhow::anyhow!("No email found in token claims"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_bearer_token_valid() {
        let mut headers = HeaderMap::new();
        headers.insert("Authorization", "Bearer my-token-123".parse().unwrap());
        assert_eq!(
            extract_bearer_token(&headers),
            Some("my-token-123".to_string())
        );
    }

    #[test]
    fn extract_bearer_token_with_whitespace() {
        let mut headers = HeaderMap::new();
        headers.insert("Authorization", "Bearer  my-token ".parse().unwrap());
        assert_eq!(
            extract_bearer_token(&headers),
            Some("my-token".to_string())
        );
    }

    #[test]
    fn extract_bearer_token_missing_header() {
        let headers = HeaderMap::new();
        assert_eq!(extract_bearer_token(&headers), None);
    }

    #[test]
    fn extract_bearer_token_wrong_scheme() {
        let mut headers = HeaderMap::new();
        headers.insert("Authorization", "Basic abc123".parse().unwrap());
        assert_eq!(extract_bearer_token(&headers), None);
    }

    #[test]
    fn extract_bearer_token_no_prefix() {
        let mut headers = HeaderMap::new();
        headers.insert("Authorization", "just-a-token".parse().unwrap());
        assert_eq!(extract_bearer_token(&headers), None);
    }

    #[test]
    fn api_key_response_serializes_correctly() {
        let resp = ApiKeyResponse {
            api_key: "test-key-uuid".to_string(),
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["api_key"], "test-key-uuid");
    }
}
