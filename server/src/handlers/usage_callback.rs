use aws_sdk_bedrockruntime::types::TokenUsage;
use sqlx::PgPool;
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info};
use usage::{CreateUsageRequest, create_usage};

pub fn create_usage_callback(
    pool: Arc<PgPool>,
    api_key: String,
    model_name: String,
) -> impl Fn(&TokenUsage) + Send + Sync + 'static {
    move |usage: &TokenUsage| {
        info!("Usage: {:?}", usage);

        let pool = pool.clone();

        let create_usage_request = CreateUsageRequest {
            api_key: api_key.clone(),
            model_name: model_name.clone(),
            total_tokens: usage.total_tokens,
        };

        tokio::spawn(async move {
            // Add timeout to prevent hanging indefinitely on database operations
            match tokio::time::timeout(
                Duration::from_secs(10),
                create_usage(&pool, &create_usage_request)
            ).await {
                Ok(Ok(_)) => {},
                Ok(Err(e)) => error!("Failed to create usage: {}", e),
                Err(_) => error!("Usage tracking timed out after 10 seconds"),
            }
        });
    }
}
