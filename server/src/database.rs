use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use std::time::Duration;
use tracing::info;

pub async fn setup_database(database_url: &str) -> anyhow::Result<PgPool> {
    info!("Connecting to database");

    let pool = PgPoolOptions::new()
        .max_connections(20)  // Increase from 5 to handle more concurrent requests
        .min_connections(2)   // Keep minimum connections ready
        .acquire_timeout(Duration::from_secs(30))  // Fail fast if pool exhausted
        .idle_timeout(Duration::from_secs(600))    // 10 minutes - release idle connections
        .max_lifetime(Duration::from_secs(1800))   // 30 minutes - prevent stale connections
        .connect(database_url)
        .await?;

    info!("Database connection established with timeout protection");

    Ok(pool)
}
