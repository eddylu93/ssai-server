pub mod token_refresh;
pub mod unit_report_job;

use anyhow::Context;
use tokio_cron_scheduler::{Job, JobScheduler};

use crate::state::AppState;

pub async fn spawn_all(state: &AppState) -> anyhow::Result<JobScheduler> {
    let scheduler = JobScheduler::new()
        .await
        .context("failed to create scheduler")?;

    let unit_report_state = state.clone();
    scheduler
        .add(
            Job::new_async("0 0/15 * * * *", move |_id, _lock| {
                let state = unit_report_state.clone();
                Box::pin(async move {
                    if let Err(error) = unit_report_job::run_once(state).await {
                        tracing::error!(error = %error, "scheduler unit_report job failed");
                    }
                })
            })
            .context("failed to create unit_report job")?,
        )
        .await
        .context("failed to register unit_report job")?;

    let refresh_state = state.clone();
    scheduler
        .add(
            Job::new_async("0 0/5 * * * *", move |_id, _lock| {
                let state = refresh_state.clone();
                Box::pin(async move {
                    if let Err(error) = token_refresh::refresh_due_accounts(state).await {
                        tracing::error!(error = %error, "scheduler token_refresh job failed");
                    }
                })
            })
            .context("failed to create token_refresh job")?,
        )
        .await
        .context("failed to register token_refresh job")?;

    scheduler
        .start()
        .await
        .context("failed to start scheduler")?;

    tracing::info!(
        "scheduler: 2 jobs registered (unit_report every 15min, token_refresh every 5min)"
    );

    Ok(scheduler)
}
