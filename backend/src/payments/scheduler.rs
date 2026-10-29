use std::time::Duration;

use chrono::{DateTime, Datelike, Timelike, Utc};
use chrono_tz::Tz;
use tokio::time::MissedTickBehavior;

use crate::{
    AppState, config::AutoPaymentReminderConfig, db::now_ts,
    notifications::payment_webhooks_configured, stats::resolve_tz,
};

use super::{dispatch_email_reminders, dispatch_webhook_reminder, selected_payment_link};

const CHECK_INTERVAL: Duration = Duration::from_secs(5 * 60);

/// Starts the background job that sends the payment reminder (webhook + email)
/// on the last day of every month once the configured local hour is reached.
pub fn spawn(state: AppState, config: AutoPaymentReminderConfig) {
    tracing::info!(
        payment_link_user_id = %config.payment_link_user_id,
        hour = config.hour,
        "automatic payment reminder enabled"
    );
    tokio::spawn(async move {
        let tz = resolve_tz(&state.config.timezone);
        let mut interval = tokio::time::interval(CHECK_INTERVAL);
        interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            if let Err(error) = run_if_due(&state, &config, Utc::now().with_timezone(&tz)).await {
                tracing::warn!(%error, "automatic payment reminder failed");
            }
        }
    });
}

/// Returns the `YYYY-MM` period to remind for when `now` is on the last day of
/// its month at or after `hour`.
pub fn due_period(now: DateTime<Tz>, hour: u32) -> Option<String> {
    let date = now.date_naive();
    let is_last_day = date
        .succ_opt()
        .is_none_or(|next| next.month() != date.month());
    (is_last_day && now.hour() >= hour).then(|| format!("{:04}-{:02}", date.year(), date.month()))
}

/// Sends the reminder if it is due and has not been sent for this month yet.
/// Returns whether this call claimed (and sent) the month's reminder.
pub async fn run_if_due(
    state: &AppState,
    config: &AutoPaymentReminderConfig,
    now: DateTime<Tz>,
) -> anyhow::Result<bool> {
    let Some(period) = due_period(now, config.hour) else {
        return Ok(false);
    };

    // Leave the month unclaimed on misconfiguration so a fixed config still sends today.
    if let Err(error) = selected_payment_link(state, &config.payment_link_user_id).await {
        tracing::warn!(
            %error,
            payment_link_user_id = %config.payment_link_user_id,
            "automatic payment reminder skipped: invalid AUTO_PAYMENT_REMINDER_LINK_USER_ID"
        );
        return Ok(false);
    }

    let claimed =
        sqlx::query("INSERT OR IGNORE INTO payment_reminder_runs (period, ran_at) VALUES (?, ?)")
            .bind(&period)
            .bind(now_ts())
            .execute(&state.db)
            .await?
            .rows_affected()
            == 1;
    if !claimed {
        return Ok(false);
    }

    tracing::info!(%period, "sending automatic payment reminder");

    if payment_webhooks_configured(state) {
        match dispatch_webhook_reminder(state, &config.payment_link_user_id).await {
            Ok(reminded_count) => {
                tracing::info!(%period, reminded_count, "automatic webhook payment reminder sent")
            }
            Err(error) => {
                tracing::info!(%period, %error, "automatic webhook payment reminder not sent")
            }
        }
    }

    if state.config.smtp.is_some() {
        match dispatch_email_reminders(state, &config.payment_link_user_id, None).await {
            Ok(delivery) => tracing::info!(
                %period,
                sent_count = delivery.sent_count,
                failed_count = delivery.failed_user_ids.len(),
                skipped_no_email_count = delivery.skipped_no_email_count,
                "automatic email payment reminders sent"
            ),
            Err(error) => {
                tracing::info!(%period, %error, "automatic email payment reminders not sent")
            }
        }
    }

    Ok(true)
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    fn at(tz: &str, y: i32, m: u32, d: u32, h: u32, min: u32) -> DateTime<Tz> {
        resolve_tz(tz)
            .with_ymd_and_hms(y, m, d, h, min, 0)
            .single()
            .unwrap()
    }

    #[test]
    fn due_on_last_day_of_month_from_configured_hour() {
        assert_eq!(
            due_period(at("UTC", 2026, 10, 31, 10, 0), 10),
            Some("2026-10".into())
        );
        assert_eq!(
            due_period(at("UTC", 2026, 10, 31, 23, 59), 10),
            Some("2026-10".into())
        );
        assert_eq!(due_period(at("UTC", 2026, 10, 31, 9, 59), 10), None);
        assert_eq!(due_period(at("UTC", 2026, 10, 30, 12, 0), 10), None);
        assert_eq!(
            due_period(at("UTC", 2026, 12, 31, 10, 0), 10),
            Some("2026-12".into())
        );
    }

    #[test]
    fn due_handles_february_and_leap_years() {
        assert_eq!(
            due_period(at("UTC", 2027, 2, 28, 10, 0), 10),
            Some("2027-02".into())
        );
        assert_eq!(due_period(at("UTC", 2028, 2, 28, 10, 0), 10), None);
        assert_eq!(
            due_period(at("UTC", 2028, 2, 29, 10, 0), 10),
            Some("2028-02".into())
        );
    }

    #[test]
    fn due_uses_local_date_in_configured_timezone() {
        // 2026-10-31 23:30 UTC is already 2026-11-01 in Berlin.
        let berlin_next_month = Utc
            .with_ymd_and_hms(2026, 10, 31, 23, 30, 0)
            .unwrap()
            .with_timezone(&resolve_tz("Europe/Berlin"));
        assert_eq!(due_period(berlin_next_month, 10), None);

        // 2026-09-30 09:00 UTC is 11:00 in Berlin.
        let berlin_last_day = Utc
            .with_ymd_and_hms(2026, 9, 30, 9, 0, 0)
            .unwrap()
            .with_timezone(&resolve_tz("Europe/Berlin"));
        assert_eq!(due_period(berlin_last_day, 10), Some("2026-09".into()));
    }
}
