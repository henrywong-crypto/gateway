use anyhow::Result;
use sqlx::PgPool;
use tracing::info;
use uuid::Uuid;

pub async fn create_inference_profile(
    model_id: &str,
    profile_name: &str,
    tags: Vec<(String, String)>,
    aws_region: &str,
    aws_account_id: &str,
    inference_profile_prefixes: &[String],
) -> Result<String> {
    let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
    let client = aws_sdk_bedrock::Client::new(&config);

    let copy_from = if inference_profile_prefixes
        .iter()
        .any(|p| model_id.starts_with(p.as_str()))
    {
        format!("arn:aws:bedrock:{aws_region}:{aws_account_id}:inference-profile/{model_id}")
    } else {
        model_id.to_string()
    };

    let mut request = client
        .create_inference_profile()
        .inference_profile_name(profile_name)
        .model_source(
            aws_sdk_bedrock::types::InferenceProfileModelSource::CopyFrom(copy_from),
        );

    for (key, value) in &tags {
        request = request.tags(
            aws_sdk_bedrock::types::Tag::builder()
                .key(key)
                .value(value)
                .build()
                .map_err(|e| anyhow::anyhow!("Failed to build tag: {}", e))?,
        );
    }

    let response = request.send().await.map_err(|e| {
        tracing::error!("Failed to create inference profile '{}': {:?}", profile_name, e);
        e
    })?;

    let arn = response.inference_profile_arn().to_string();

    info!("Created inference profile: {} (ARN: {})", profile_name, arn);

    Ok(arn)
}

pub async fn create_inference_profile_record(
    pool: &PgPool,
    user_email: &str,
    model_name: &str,
    inference_profile_arn: &str,
    inference_profile_name: &str,
) -> Result<()> {
    sqlx::query!(
        r#"
        INSERT INTO inference_profiles (user_id, model_id, inference_profile_arn, inference_profile_name)
        SELECT u.user_id, m.model_id, $3, $4
        FROM users u, models m
        WHERE u.email = $1 AND m.model_name = $2
        "#,
        user_email.to_lowercase(),
        model_name.to_lowercase(),
        inference_profile_arn,
        inference_profile_name,
    )
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn create_and_store_inference_profile(
    pool: &PgPool,
    api_key: &str,
    model_name: &str,
    aws_region: &str,
    aws_account_id: &str,
    inference_profile_prefixes: &[String],
) -> Result<String> {
    let profile_name = Uuid::new_v4().to_string();
    let arn = create_inference_profile(model_name, &profile_name, vec![], aws_region, aws_account_id, inference_profile_prefixes).await?;

    sqlx::query!(
        r#"
        INSERT INTO inference_profiles (user_id, model_id, inference_profile_arn, inference_profile_name)
        SELECT ak.user_id, m.model_id, $3, $4
        FROM api_keys ak, models m
        WHERE ak.api_key = $1 AND m.model_name = $2
        "#,
        api_key,
        model_name,
        &arn,
        &profile_name,
    )
    .execute(pool)
    .await?;

    info!(
        "Created and stored inference profile: {} (ARN: {})",
        profile_name, arn
    );

    Ok(arn)
}
