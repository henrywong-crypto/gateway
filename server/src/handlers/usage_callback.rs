use aws_sdk_bedrockruntime::types::TokenUsage;
use tracing::info;

pub fn create_usage_callback() -> impl Fn(&TokenUsage) + Send + Sync + 'static {
    move |usage: &TokenUsage| {
        info!("Usage: {:?}", usage);
    }
}
