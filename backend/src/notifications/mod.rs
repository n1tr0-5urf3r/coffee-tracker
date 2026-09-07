use std::time::Duration;

use serde::Serialize;

use crate::{AppState, achievements::AchievementEvent, auth::CurrentUser};

pub mod email;

#[derive(Serialize)]
struct WebhookPayload<'a> {
    text: &'a str,
}

struct WebhookDestination<'a> {
    name: &'static str,
    url: &'a str,
}

pub async fn notify_unlocks(state: &AppState, user: &CurrentUser, unlocks: &[AchievementEvent]) {
    let destinations = webhook_destinations(state);
    if destinations.is_empty() {
        tracing::debug!(
            user_id = %user.id,
            "webhooks not configured; skipping notify_unlocks",
        );
        return;
    }
    if unlocks.is_empty() {
        tracing::debug!(user_id = %user.id, "no achievements unlocked; skipping webhooks");
        return;
    }
    if !user.mattermost_opt_in {
        tracing::debug!(
            user_id = %user.id,
            "user opted out of webhook notifications; skipping",
        );
        return;
    }
    if user.achievement_sharing_opt_out {
        tracing::debug!(
            user_id = %user.id,
            "user opted out of achievement sharing; skipping webhooks",
        );
        return;
    }

    tracing::info!(
        user_id = %user.id,
        count = unlocks.len(),
        destinations = destinations.len(),
        "posting achievement unlocks to webhooks",
    );
    for unlock in unlocks {
        let text = achievement_message(&user.display_name, unlock);
        post_webhooks(state, &destinations, &text, "achievement").await;
    }
}

fn webhook_destinations(state: &AppState) -> Vec<WebhookDestination<'_>> {
    let mut destinations = Vec::new();
    if let Some(url) = &state.config.mattermost_webhook_url {
        destinations.push(WebhookDestination {
            name: "Mattermost",
            url,
        });
    }
    if let Some(url) = &state.config.matrix_webhook_url {
        destinations.push(WebhookDestination {
            name: "Matrix",
            url,
        });
    }
    destinations
}

pub fn payment_webhooks_configured(state: &AppState) -> bool {
    !webhook_destinations(state).is_empty()
}

async fn post_webhooks(
    state: &AppState,
    destinations: &[WebhookDestination<'_>],
    text: &str,
    kind: &str,
) {
    for destination in destinations {
        post_webhook(state, destination, text, kind).await;
    }
}

async fn post_webhook(
    state: &AppState,
    destination: &WebhookDestination<'_>,
    text: &str,
    kind: &str,
) {
    match state
        .http
        .post(destination.url)
        .timeout(Duration::from_secs(20))
        .json(&WebhookPayload { text })
        .send()
        .await
    {
        Ok(response) => {
            let status = response.status();
            if !status.is_success() {
                let body = response.text().await.unwrap_or_default();
                tracing::warn!(
                    %status,
                    body = %body,
                    kind = %kind,
                    destination = destination.name,
                    "webhook rejected payload",
                );
            } else {
                tracing::info!(
                    %status,
                    kind = %kind,
                    destination = destination.name,
                    "webhook delivered"
                );
            }
        }
        Err(err) => {
            tracing::warn!(
                ?err,
                kind = %kind,
                destination = destination.name,
                "failed to reach webhook"
            );
        }
    }
}

fn achievement_message(display_name: &str, unlock: &AchievementEvent) -> String {
    format!(
        "🏆 **{} unlocked achievement: {}**\n{}",
        display_name, unlock.name, unlock.description
    )
}

pub async fn notify_stock_low(
    state: &AppState,
    item_name: &str,
    current_amount: f64,
    critical_amount: f64,
    unit: &str,
) {
    let destinations = webhook_destinations(state);
    if destinations.is_empty() {
        tracing::debug!(item = %item_name, "webhooks not configured; skipping stock_low");
        return;
    }
    tracing::info!(
        item = %item_name,
        destinations = destinations.len(),
        "posting stock-low reminder to webhooks"
    );
    let text = format!(
        "⚠️ Coffee stock reminder: {item_name} is low ({:.2} {unit} left, threshold {:.2} {unit}). Please order more.",
        current_amount, critical_amount
    );
    post_webhooks(state, &destinations, &text, "stock_low").await;
}

pub async fn notify_payment_marked_paid(state: &AppState, display_name: &str, amount_cents: i64) {
    let destinations = webhook_destinations(state);
    if destinations.is_empty() {
        tracing::debug!("webhooks not configured; skipping payment_paid");
        return;
    }
    tracing::info!(
        destinations = destinations.len(),
        "posting payment-paid notice to webhooks"
    );
    let text = payment_marked_paid_message(display_name, amount_cents);
    post_webhooks(state, &destinations, &text, "payment_paid").await;
}

pub async fn notify_payment_reminder(state: &AppState, text: &str) {
    let destinations = webhook_destinations(state);
    if destinations.is_empty() {
        tracing::debug!("webhooks not configured; skipping payment_reminder");
        return;
    }
    tracing::info!(
        destinations = destinations.len(),
        "posting payment reminder to webhooks"
    );
    post_webhooks(state, &destinations, text, "payment_reminder").await;
}

fn payment_marked_paid_message(display_name: &str, amount_cents: i64) -> String {
    format!(
        "💸 **{} marked their debt of {} as paid.** ☕",
        display_name,
        format_euros(amount_cents)
    )
}

fn format_euros(amount_cents: i64) -> String {
    format!("{}.{:02} EUR", amount_cents / 100, amount_cents.abs() % 100)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn achievement_message_is_webhook_markdown() {
        let message = achievement_message(
            "Ada",
            &AchievementEvent {
                id: "first_eis".into(),
                name: "Eis Break".into(),
                description: "Logged the first Eis.".into(),
            },
        );

        assert_eq!(
            message,
            "🏆 **Ada unlocked achievement: Eis Break**\nLogged the first Eis."
        );
    }

    #[test]
    fn payment_marked_paid_message_is_webhook_markdown() {
        assert_eq!(
            payment_marked_paid_message("Ada", 1220),
            "💸 **Ada marked their debt of 12.20 EUR as paid.** ☕"
        );
    }
}
