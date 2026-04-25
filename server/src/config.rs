use std::{env, net::SocketAddr};

use anyhow::Context;

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub pg_max_conn: u32,
    pub bind_addr: SocketAddr,
    pub rust_log: String,
    pub log_format: String,
    pub version: String,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        let database_url = required_var("DATABASE_URL")?;
        let pg_max_conn = env::var("PG_MAX_CONN")
            .unwrap_or_else(|_| "10".to_string())
            .parse()
            .context("PG_MAX_CONN must be a positive integer")?;
        let bind_addr = env::var("BIND_ADDR")
            .unwrap_or_else(|_| "127.0.0.1:8080".to_string())
            .parse()
            .context("BIND_ADDR must be a valid socket address")?;
        let rust_log = env::var("RUST_LOG")
            .unwrap_or_else(|_| "ssai_server=info,tower_http=warn,sqlx=warn".to_string());
        let log_format = env::var("LOG_FORMAT").unwrap_or_else(|_| "pretty".to_string());
        let version = env!("CARGO_PKG_VERSION").to_string();

        Ok(Self {
            database_url,
            pg_max_conn,
            bind_addr,
            rust_log,
            log_format,
            version,
        })
    }
}

fn required_var(name: &str) -> anyhow::Result<String> {
    env::var(name).with_context(|| format!("{name} must be set"))
}
