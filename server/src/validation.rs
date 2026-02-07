use sqlx::PgPool;

pub struct ValidationResult {
    pub api_key_exists: bool,
    pub model_exists: bool,
    pub inference_profile_arn: Option<String>,
    pub user_email: Option<String>,
}

pub async fn check_api_key_exists_and_model_exists(
    pool: &PgPool,
    api_key: &str,
    model_name: &str,
) -> anyhow::Result<ValidationResult> {
    let result = sqlx::query!(
        r#"
        SELECT
            EXISTS (SELECT 1 FROM api_keys WHERE api_key = $1 AND is_disabled = FALSE) as "api_key_exists!",
            EXISTS (SELECT 1 FROM models WHERE model_name = $2) as "model_exists!",
            (
                SELECT ip.inference_profile_arn
                FROM inference_profiles ip
                JOIN api_keys ak ON ip.user_id = ak.user_id
                JOIN models m ON ip.model_id = m.model_id
                WHERE ak.api_key = $1 AND ak.is_disabled = false AND m.model_name = $2
            ) as inference_profile_arn,
            (
                SELECT u.email
                FROM users u
                JOIN api_keys ak ON u.user_id = ak.user_id
                WHERE ak.api_key = $1 AND ak.is_disabled = false
            ) as user_email
        "#,
        api_key.to_lowercase(),
        model_name.to_lowercase()
    )
    .fetch_one(pool)
    .await?;

    Ok(ValidationResult {
        api_key_exists: result.api_key_exists,
        model_exists: result.model_exists,
        inference_profile_arn: result.inference_profile_arn,
        user_email: result.user_email,
    })
}

pub async fn check_api_key_exists(pool: &PgPool, api_key: &str) -> anyhow::Result<bool> {
    let result = sqlx::query_scalar!(
        r#"
        SELECT EXISTS (SELECT 1 FROM api_keys WHERE api_key = $1 AND is_disabled = FALSE)
        "#,
        api_key.to_lowercase()
    )
    .fetch_one(pool)
    .await?;

    Ok(result.unwrap_or(false))
}
