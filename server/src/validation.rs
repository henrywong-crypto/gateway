use sqlx::PgPool;

pub async fn check_api_key_exists_and_model_exists(
    pool: &PgPool,
    api_key: &str,
    model_name: &str,
) -> anyhow::Result<(bool, bool, Option<String>)> {
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
                WHERE ak.api_key = $1 AND m.model_name = $2
                LIMIT 1
            ) as inference_profile_arn
        "#,
        api_key.to_lowercase(),
        model_name.to_lowercase()
    )
    .fetch_one(pool)
    .await?;

    Ok((result.api_key_exists, result.model_exists, result.inference_profile_arn))
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
