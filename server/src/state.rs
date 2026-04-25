use std::sync::Arc;

use anyhow::Context;
use reqwest::Client;
use sqlx::{postgres::PgPoolOptions, PgPool};

use crate::{config::Config, crypto::aead::Aead256};

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub http: Client,
    pub aead: Arc<Aead256>,
    pub config: Arc<Config>,
}

impl AppState {
    pub async fn from_config(config: Config) -> anyhow::Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(config.pg_max_conn)
            .connect(&config.database_url)
            .await
            .context("failed to connect to PostgreSQL")?;

        let http = Client::builder()
            .use_rustls_tls()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .context("failed to build reqwest client")?;

        let aead = Arc::new(Aead256::from_base64_key(&config.token_enc_key)?);

        Ok(Self {
            pool,
            http,
            aead,
            config: Arc::new(config),
        })
    }
}
