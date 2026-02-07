use serde::{Deserialize, Serialize};
use sqlx::PgPool;

#[derive(Deserialize)]
pub struct Model {
    pub model_arn: String,
    pub protected: bool,
}

#[derive(Serialize)]
pub struct Data {
    pub created: i64,
    pub id: String,
    pub object: String,
    pub owned_by: String,
}

#[derive(Serialize)]
pub struct ModelsResponse {
    pub data: Vec<Data>,
    pub object: String,
}

pub async fn get_models(pool: &PgPool) -> anyhow::Result<Vec<Model>> {
    let models = sqlx::query_as!(
        Model,
        r#"
        SELECT
            model_arn,
            protected
        FROM models
        ORDER BY model_arn
        "#
    )
    .fetch_all(pool)
    .await?;

    Ok(models)
}

pub async fn create_model(pool: &PgPool, model_arn: &str) -> anyhow::Result<()> {
    sqlx::query!(
        r#"
        INSERT INTO models (model_arn)
        VALUES ($1)
        "#,
        model_arn.to_lowercase()
    )
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn delete_model(pool: &PgPool, model_arn: &str) -> anyhow::Result<()> {
    sqlx::query!(
        r#"
        DELETE FROM models
        WHERE model_arn = $1 AND protected = false
        "#,
        model_arn.to_lowercase()
    )
    .execute(pool)
    .await?;

    Ok(())
}

pub fn to_models_response(models: &[Model]) -> ModelsResponse {
    let data = models
        .iter()
        .map(|model| Data {
            created: 0,
            id: model.model_arn.clone(),
            object: "model".to_string(),
            owned_by: "".to_string(),
        })
        .collect();

    ModelsResponse {
        data,
        object: "list".to_string(),
    }
}
