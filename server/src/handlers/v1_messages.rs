use std::time::Duration;

use anthropic_request::V1MessagesRequest;
use anyhow::Context;
use apikeys::get_api_key;
use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::sse::Sse,
};
use chat::provider::{BedrockV1MessagesProvider, V1MessagesProvider};
use inference_profiles::get_inference_profile_arn;
use myerrors::AppError;
use myhandlers::AppState;
use tokio_stream::StreamExt as _;
use tracing::{debug, error};

use crate::validation::check_api_key_exists_and_model_exists;

use super::usage_callback::create_usage_callback;

pub async fn v1_messages(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(mut payload): Json<V1MessagesRequest>,
) -> Result<impl axum::response::IntoResponse, AppError> {
    debug!("Received v1/messages request for model: {}", payload.model);

    let api_key = get_api_key(&headers)
        .await
        .context("Missing API key (provide Authorization: Bearer <key> or x-api-key header)")?;

    payload.model = payload.model.to_lowercase();

    let (api_key_exists, model_exists) =
        check_api_key_exists_and_model_exists(&state.db_pool, &api_key, &payload.model).await?;

    if !api_key_exists {
        error!("API key validation failed: Invalid API key");
        return Err(AppError::from(anyhow::anyhow!(
            "Invalid or missing API key"
        )));
    }

    if !model_exists {
        error!("Model name validation failed: Invalid model name");
        return Err(AppError::from(anyhow::anyhow!(
            "Invalid or missing model name"
        )));
    }

    if payload.stream == Some(false) {
        error!("Streaming is required but was disabled by client (stream: false)");
        return Err(AppError::from(anyhow::anyhow!(
            "Streaming is required but was disabled"
        )));
    }

    if let Ok(Some(arn)) =
        get_inference_profile_arn(&state.db_pool, &api_key, &payload.model).await
    {
        debug!(
            "Using inference profile ARN: {} for model: {}",
            arn, payload.model
        );
        payload.model = arn;
    }

    let usage_callback = create_usage_callback();

    let stream = BedrockV1MessagesProvider::new()
        .await
        .v1_messages_stream(payload, usage_callback)
        .await?;

    let stream = stream.timeout(Duration::from_secs(30)).map(|item| match item {
        Ok(inner) => inner,
        Err(_elapsed) => Err(anyhow::anyhow!("Stream item timed out")),
    });

    Ok((StatusCode::OK, Sse::new(stream)))
}
