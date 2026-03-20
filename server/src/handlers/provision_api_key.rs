use apikeys::{create_api_key, disable_all_api_keys, get_active_api_key};
use axum::{
    Json,
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use jsonwebtoken::{DecodingKey, Validation, decode, decode_header};
use myhandlers::AppState;
use serde::{Deserialize, Serialize};
use tracing::{error, info};

#[derive(Deserialize)]
pub struct ProvisionQuery {
    pub force_new: Option<bool>,
}

#[derive(Serialize)]
struct ApiKeyResponse {
    api_key: String,
}

#[derive(Deserialize)]
struct JwksResponse {
    keys: Vec<Jwk>,
}

#[derive(Deserialize)]
struct Jwk {
    kid: String,
    n: String,
    e: String,
    kty: String,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct CognitoClaims {
    sub: Option<String>,
    email: Option<String>,
    username: Option<String>,
    token_use: Option<String>,
    iss: Option<String>,
    exp: Option<u64>,
}

/// POST /api/v1/api-keys
///
/// Accepts `Authorization: Bearer <cognito_access_token>`.
/// Validates the JWT against gateway Cognito JWKS, extracts the user email,
/// creates the user if needed, and returns an existing active API key or
/// creates a new one. Pass `?force_new=true` to disable existing keys and
/// generate a fresh one.
pub async fn provision_api_key(
    headers: HeaderMap,
    query: Query<ProvisionQuery>,
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

    let force_new = query.force_new.unwrap_or(false);

    if force_new {
        if let Err(e) = disable_all_api_keys(&state.db_pool, &email).await {
            error!("disable_all_api_keys failed: {e}");
            return Err((StatusCode::INTERNAL_SERVER_ERROR, "Internal error").into_response());
        }
    } else {
        // Check for existing active key
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
    let issuer = format!(
        "https://cognito-idp.{}.amazonaws.com/{}",
        state.cognito_region, state.cognito_user_pool_id
    );
    let jwks_url = format!("{}/.well-known/jwks.json", issuer);

    // Decode header to get kid
    let header = decode_header(token)?;
    let kid = header.kid.ok_or_else(|| anyhow::anyhow!("JWT missing kid"))?;

    // Fetch JWKS
    let jwks: JwksResponse = reqwest::get(&jwks_url).await?.json().await?;

    let jwk = jwks
        .keys
        .iter()
        .find(|k| k.kid == kid)
        .ok_or_else(|| anyhow::anyhow!("No matching key found in JWKS"))?;

    if jwk.kty != "RSA" {
        return Err(anyhow::anyhow!("Unsupported key type: {}", jwk.kty));
    }

    let decoding_key = DecodingKey::from_rsa_components(&jwk.n, &jwk.e)?;

    let mut validation = Validation::new(jsonwebtoken::Algorithm::RS256);
    validation.set_issuer(&[&issuer]);
    // Cognito access tokens don't have aud claim, so disable audience validation
    validation.validate_aud = false;

    let token_data = decode::<CognitoClaims>(token, &decoding_key, &validation)?;

    // Try to get email from claims first
    if let Some(email) = token_data.claims.email {
        if !email.is_empty() {
            return Ok(email);
        }
    }

    // For access tokens, email might not be in claims — call userInfo endpoint
    let userinfo_url = format!(
        "https://{}.auth.{}.amazoncognito.com/oauth2/userInfo",
        state.cognito_domain, state.cognito_region
    );

    let client = reqwest::Client::new();
    let userinfo_resp: serde_json::Value = client
        .get(&userinfo_url)
        .bearer_auth(token)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    userinfo_resp
        .get("email")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow::anyhow!("No email found in token claims or userInfo"))
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
    fn provision_query_defaults_force_new_to_false() {
        let query = ProvisionQuery { force_new: None };
        assert!(!query.force_new.unwrap_or(false));
    }

    #[test]
    fn provision_query_force_new_true() {
        let query = ProvisionQuery {
            force_new: Some(true),
        };
        assert!(query.force_new.unwrap_or(false));
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
