use std::time::Duration;

use anyhow::Context;
use apikeys::get_api_key;
use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, sse::Sse},
};
use chat::bedrock::ReasoningEffortToThinkingBudgetTokens;
use chat::provider::{BedrockChatCompletionsProvider, ChatCompletionsProvider};
use inference_profiles::{create_inference_profile, create_inference_profile_record};
use myerrors::AppError;
use myhandlers::AppState;
use request::ChatCompletionsRequest;
use tokio_stream::StreamExt as _;
use tracing::{debug, error};

use crate::validation::check_api_key_exists_and_model_exists;

use super::usage_callback::create_usage_callback;

pub async fn chat_completions(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(mut payload): Json<ChatCompletionsRequest>,
) -> Result<impl IntoResponse, AppError> {
    debug!(
        "Received chat completions request for model: {}",
        payload.model
    );

    let api_key = get_api_key(&headers)
        .await
        .context("Missing API key (provide Authorization: Bearer <key> or x-api-key header)")?;

    payload.model = payload.model.to_lowercase();

    let validation =
        check_api_key_exists_and_model_exists(&state.db_pool, &api_key, &payload.model).await?;

    if !validation.api_key_exists {
        error!("API key validation failed: Invalid API key");
        return Err(AppError::from(anyhow::anyhow!(
            "Invalid or missing API key"
        )));
    }

    if !validation.model_exists {
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

    let arn = match validation.inference_profile_arn {
        Some(arn) => Some(arn),
        None => {
            if let Some(ref user_email) = validation.user_email {
                let profile_name = uuid::Uuid::new_v4().to_string();
                let tags = vec![("user_email".to_string(), user_email.clone())];
                match create_inference_profile(&payload.model, &profile_name, tags).await {
                    Ok(arn) => {
                        let _ = create_inference_profile_record(
                            &state.db_pool,
                            user_email,
                            &payload.model,
                            &arn,
                            &profile_name,
                        )
                        .await;
                        Some(arn)
                    }
                    Err(e) => {
                        error!("Failed to create inference profile for model '{}': {:?}", payload.model, e);
                        None
                    }
                }
            } else {
                None
            }
        }
    };

    if let Some(arn) = arn {
        debug!(
            "Using inference profile ARN: {} for model: {}",
            arn, payload.model
        );
        payload.model = arn;
    }

    let usage_callback = create_usage_callback();

    let reasoning_effort_to_thinking_budget_tokens =
        ReasoningEffortToThinkingBudgetTokens::default();

    let stream = BedrockChatCompletionsProvider::new()
        .await
        .chat_completions_stream(
            payload,
            reasoning_effort_to_thinking_budget_tokens,
            usage_callback,
        )
        .await?;

    let stream = stream.timeout(Duration::from_secs(30)).map(|item| match item {
        Ok(inner) => inner,
        Err(_elapsed) => Err(anyhow::anyhow!("Stream item timed out")),
    });

    Ok((StatusCode::OK, Sse::new(stream)))
}
