use anyhow::Context;
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHasher, SaltString},
    Algorithm, Argon2, Params, Version,
};
use sqlx::query;

use crate::state::AppState;

pub async fn bootstrap_admin_user(state: &AppState) -> anyhow::Result<()> {
    let Some(email) = state.config.bootstrap_admin_email.as_deref() else {
        tracing::info!("BOOTSTRAP_ADMIN_EMAIL not set; skipping bootstrap admin");
        return Ok(());
    };
    let Some(password) = state.config.bootstrap_admin_password.as_deref() else {
        tracing::info!("BOOTSTRAP_ADMIN_PASSWORD not set; skipping bootstrap admin");
        return Ok(());
    };

    let argon2 = Argon2::new(
        Algorithm::Argon2id,
        Version::V0x13,
        Params::new(65_536, 3, 4, None).map_err(|_| anyhow::anyhow!("invalid argon2 params"))?,
    );
    let salt = SaltString::generate(&mut OsRng);
    let password_hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|_| anyhow::anyhow!("failed to hash bootstrap admin password"))?
        .to_string();

    query(
        r#"
        INSERT INTO users (email, password_hash, role, disabled)
        VALUES ($1, $2, 'admin', false)
        ON CONFLICT (email) DO UPDATE
        SET password_hash = EXCLUDED.password_hash,
            role = 'admin',
            disabled = false
        "#,
    )
    .bind(email)
    .bind(password_hash)
    .execute(&state.pool)
    .await
    .context("failed to upsert bootstrap admin user")?;

    tracing::info!(email = email, "bootstrap admin user ensured");
    Ok(())
}
