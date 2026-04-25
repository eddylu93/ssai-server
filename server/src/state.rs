use anyhow::Context;
use sqlx::{postgres::PgPoolOptions, PgPool};

use crate::config::Config;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub version: String,
}

impl AppState {
    pub async fn from_config(config: Config) -> anyhow::Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(config.pg_max_conn)
            .connect(&config.database_url)
            .await
            .context("failed to connect to PostgreSQL")?;

        Ok(Self {
            pool,
            version: config.version,
        })
    }
}
