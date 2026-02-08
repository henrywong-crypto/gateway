use anyhow::Result;
use sqlx::PgPool;
use tracing::{error, info};
use uuid::Uuid;

pub async fn create_and_store_inference_profile(
    pool: &PgPool,
    api_key: &str,
    model_name: &str,
    aws_region: &str,
    aws_account_id: &str,
    inference_profile_prefixes: &[String],
) -> Result<String> {
    let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
    let client = aws_sdk_bedrock::Client::new(&config);

    let copy_from = if inference_profile_prefixes
        .iter()
        .any(|p| model_name.starts_with(p.as_str()))
    {
        format!("arn:aws:bedrock:{aws_region}:{aws_account_id}:inference-profile/{model_name}")
    } else {
        model_name.to_string()
    };

    let profile_name = Uuid::new_v4().to_string();

    let response = client
        .create_inference_profile()
        .inference_profile_name(&profile_name)
        .model_source(
            aws_sdk_bedrock::types::InferenceProfileModelSource::CopyFrom(copy_from),
        )
        .send()
        .await
        .map_err(|e| {
            error!("Failed to create inference profile '{}': {:?}", profile_name, e);
            e
        })?;

    let arn = response.inference_profile_arn().to_string();

    let insert_result = sqlx::query!(
        r#"
        INSERT INTO inference_profiles (user_id, model_id, inference_profile_arn, inference_profile_name)
        SELECT ak.user_id, m.model_id, $3, $4
        FROM api_keys ak, models m
        WHERE ak.api_key = $1 AND m.model_name = $2
        "#,
        api_key.to_lowercase(),
        model_name.to_lowercase(),
        &arn,
        &profile_name,
    )
    .execute(pool)
    .await;

    if let Err(e) = insert_result {
        error!("Failed to store inference profile record, deleting AWS profile: {}", e);
        if let Err(delete_err) = client
            .delete_inference_profile()
            .inference_profile_identifier(&arn)
            .send()
            .await
        {
            error!("Failed to delete orphaned inference profile '{}': {:?}", arn, delete_err);
        }
        return Err(e.into());
    }

    info!(
        "Created and stored inference profile: {} (ARN: {})",
        profile_name, arn
    );

    Ok(arn)
}
