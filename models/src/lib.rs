use serde::{Deserialize, Serialize};
use sqlx::PgPool;

#[derive(Deserialize)]
pub struct Model {
    pub model_name: String,
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
            model_name
        FROM models
        "#
    )
    .fetch_all(pool)
    .await?;

    Ok(models)
}

pub fn to_models_response(models: &Vec<Model>) -> ModelsResponse {
    let data = models
        .iter()
        .map(|model| Data {
            created: 0,
            id: model.model_name.clone(),
            object: "model".to_string(),
            owned_by: "".to_string(),
        })
        .collect();

    ModelsResponse {
        data,
        object: "list".to_string(),
    }
}
