use std::collections::HashSet;

use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    AppState,
    achievements::evaluate_after_payment,
    auth::CurrentUser,
    db::now_ts,
    error::{ApiError, ApiResult},
    expenses::{
        user_expense_balance_cents, user_expense_total_cents, user_received_payment_total_cents,
    },
    loyalty::{discounted_amount_cents, paid_drinks_since_month_of},
    notifications::{
        email::{PaymentReminderEmail, email_address_available, send_payment_reminder_emails},
        notify_payment_marked_paid, notify_payment_reminder, notify_unlocks,
        payment_webhooks_configured,
    },
    stats::resolve_tz,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/payments", get(list_payments).post(mark_paid))
        .route("/payments/me", get(my_payments))
        .route("/payments/config", get(payment_config))
        .route("/payments/links", get(list_paypal_links))
        .route("/payments/recipients", get(list_payment_recipients))
        .route("/payments/reminders", post(send_payment_reminder))
        .route(
            "/payments/email-reminders",
            post(send_email_payment_reminder),
        )
}

#[derive(Serialize, ToSchema)]
pub struct PaypalLinkEntry {
    pub user_id: String,
    pub display_name: String,
    pub paypal_link: String,
}

#[derive(Serialize, ToSchema)]
pub struct PaypalLinksResponse {
    pub entries: Vec<PaypalLinkEntry>,
}

#[derive(Serialize, ToSchema)]
pub struct PaymentRecipient {
    pub user_id: String,
    pub display_name: String,
    pub paypal_link: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct PaymentRecipientsResponse {
    pub entries: Vec<PaymentRecipient>,
}

#[utoipa::path(get, path = "/api/v1/payments/links", responses((status = 200, body = PaypalLinksResponse)))]
pub(crate) async fn list_paypal_links(
    State(state): State<AppState>,
    _user: CurrentUser,
) -> ApiResult<Json<PaypalLinksResponse>> {
    let rows: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT id, display_name, paypal_link
         FROM users
         WHERE paypal_link IS NOT NULL
           AND TRIM(paypal_link) <> ''
           AND stock_manager = 1
         ORDER BY display_name ASC",
    )
    .fetch_all(&state.db)
    .await?;

    let entries = rows
        .into_iter()
        .map(|(id, display_name, paypal_link)| PaypalLinkEntry {
            user_id: id,
            display_name,
            paypal_link,
        })
        .collect();

    Ok(Json(PaypalLinksResponse { entries }))
}

#[utoipa::path(get, path = "/api/v1/payments/recipients", responses((status = 200, body = PaymentRecipientsResponse)))]
pub(crate) async fn list_payment_recipients(
    State(state): State<AppState>,
    _user: CurrentUser,
) -> ApiResult<Json<PaymentRecipientsResponse>> {
    let rows: Vec<(String, String, Option<String>)> = sqlx::query_as(
        "SELECT id, display_name, paypal_link
         FROM users
         WHERE stock_manager = 1
         ORDER BY display_name ASC",
    )
    .fetch_all(&state.db)
    .await?;

    let entries = rows
        .into_iter()
        .map(|(id, display_name, paypal_link)| PaymentRecipient {
            user_id: id,
            display_name,
            paypal_link: paypal_link.and_then(|value| (!value.trim().is_empty()).then(|| value)),
        })
        .collect();

    Ok(Json(PaymentRecipientsResponse { entries }))
}

#[derive(Serialize, ToSchema)]
pub struct PaymentSummary {
    pub user_id: String,
    pub display_name: String,
    pub user_type: String,
    pub unpaid_amount_cents: i64,
    pub unpaid_drink_count: i64,
    pub paid_amount_cents: i64,
    pub paid_drink_count: i64,
    pub expense_amount_cents: i64,
    pub received_payment_amount_cents: i64,
    pub expense_balance_cents: i64,
    pub last_paid_at: Option<i64>,
    pub email_available: bool,
}

#[derive(Serialize, ToSchema)]
pub struct PaymentsResponse {
    pub entries: Vec<PaymentSummary>,
    pub received_payments: Vec<ReceivedPaymentEntry>,
}

#[derive(Serialize, ToSchema)]
pub struct MyPaymentResponse {
    pub user_id: String,
    pub unpaid_amount_cents: i64,
    pub unpaid_drink_count: i64,
    pub paid_amount_cents: i64,
    pub paid_drink_count: i64,
    pub expense_amount_cents: i64,
    pub received_payment_amount_cents: i64,
    pub expense_balance_cents: i64,
    pub last_paid_at: Option<i64>,
}

#[derive(Serialize, ToSchema)]
pub struct PaymentConfigResponse {
    pub paypal_link: Option<String>,
    pub email_reminders_configured: bool,
}

#[derive(Deserialize, ToSchema)]
pub struct MarkPaidRequest {
    pub user_id: String,
    pub received_by_user_id: String,
    pub note: Option<String>,
}

#[derive(Deserialize, ToSchema)]
pub struct PaymentReminderRequest {
    pub payment_link_user_id: String,
}

#[derive(Serialize, ToSchema)]
pub struct PaymentReminderResponse {
    pub reminded_count: usize,
}

#[derive(Deserialize, ToSchema)]
pub struct EmailPaymentReminderRequest {
    pub payment_link_user_id: String,
    pub recipient_user_ids: Option<Vec<String>>,
}

#[derive(Serialize, ToSchema)]
pub struct EmailPaymentReminderResponse {
    pub sent_count: usize,
    pub failed_user_ids: Vec<String>,
    pub skipped_no_email_count: usize,
}

#[derive(Serialize, ToSchema)]
pub struct PaymentRecord {
    pub id: String,
    pub user_id: String,
    pub received_by_user_id: Option<String>,
    pub amount_cents: i64,
    pub drink_count: i64,
    pub period_end_ts: i64,
    pub note: Option<String>,
    pub paid_at: i64,
}

#[derive(Serialize, ToSchema)]
pub struct ReceivedPaymentEntry {
    pub id: String,
    pub payer_user_id: String,
    pub payer_display_name: String,
    pub recipient_user_id: String,
    pub recipient_display_name: String,
    pub amount_cents: i64,
    pub drink_count: i64,
    pub paid_at: i64,
    pub note: Option<String>,
}

#[utoipa::path(get, path = "/api/v1/payments", responses((status = 200, body = PaymentsResponse)))]
pub(crate) async fn list_payments(
    State(state): State<AppState>,
    user: CurrentUser,
) -> ApiResult<Json<PaymentsResponse>> {
    ensure_stock_manager(&user)?;

    let users: Vec<UserRow> = sqlx::query_as(
        "SELECT id, display_name, user_type, email FROM users ORDER BY display_name ASC",
    )
    .fetch_all(&state.db)
    .await?;

    let mut entries = Vec::with_capacity(users.len());
    for user in users {
        let totals = user_totals(&state, &user.id).await?;
        let expense_amount_cents = user_expense_total_cents(&state, &user.id).await?;
        let received_payment_amount_cents =
            user_received_payment_total_cents(&state, &user.id).await?;
        let expense_balance_cents = user_expense_balance_cents(&state, &user.id).await?;
        entries.push(PaymentSummary {
            user_id: user.id,
            display_name: user.display_name,
            user_type: user.user_type,
            unpaid_amount_cents: totals.unpaid_amount_cents,
            unpaid_drink_count: totals.unpaid_drink_count,
            paid_amount_cents: totals.paid_amount_cents,
            paid_drink_count: totals.paid_drink_count,
            expense_amount_cents,
            received_payment_amount_cents,
            expense_balance_cents,
            last_paid_at: totals.last_paid_at,
            email_available: email_address_available(user.email.as_deref()),
        });
    }

    entries.sort_by(|a, b| {
        amount_due_cents(b.unpaid_amount_cents, b.expense_balance_cents)
            .cmp(&amount_due_cents(
                a.unpaid_amount_cents,
                a.expense_balance_cents,
            ))
            .then_with(|| b.unpaid_amount_cents.cmp(&a.unpaid_amount_cents))
            .then_with(|| a.display_name.cmp(&b.display_name))
    });

    let received_payments = received_payment_entries(&state, &user.id).await?;

    Ok(Json(PaymentsResponse {
        entries,
        received_payments,
    }))
}

#[utoipa::path(get, path = "/api/v1/payments/me", responses((status = 200, body = MyPaymentResponse)))]
pub(crate) async fn my_payments(
    State(state): State<AppState>,
    user: CurrentUser,
) -> ApiResult<Json<MyPaymentResponse>> {
    let totals = user_totals(&state, &user.id).await?;
    let expense_amount_cents = user_expense_total_cents(&state, &user.id).await?;
    let received_payment_amount_cents = user_received_payment_total_cents(&state, &user.id).await?;
    let expense_balance_cents = user_expense_balance_cents(&state, &user.id).await?;
    Ok(Json(MyPaymentResponse {
        user_id: user.id,
        unpaid_amount_cents: totals.unpaid_amount_cents,
        unpaid_drink_count: totals.unpaid_drink_count,
        paid_amount_cents: totals.paid_amount_cents,
        paid_drink_count: totals.paid_drink_count,
        expense_amount_cents,
        received_payment_amount_cents,
        expense_balance_cents,
        last_paid_at: totals.last_paid_at,
    }))
}

#[utoipa::path(get, path = "/api/v1/payments/config", responses((status = 200, body = PaymentConfigResponse)))]
pub(crate) async fn payment_config(
    State(state): State<AppState>,
    _user: CurrentUser,
) -> ApiResult<Json<PaymentConfigResponse>> {
    Ok(Json(PaymentConfigResponse {
        paypal_link: state.config.paypal_link.clone(),
        email_reminders_configured: state.config.smtp.is_some(),
    }))
}

#[utoipa::path(
    post,
    path = "/api/v1/payments/reminders",
    request_body = PaymentReminderRequest,
    responses((status = 200, body = PaymentReminderResponse))
)]
pub(crate) async fn send_payment_reminder(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(input): Json<PaymentReminderRequest>,
) -> ApiResult<Json<PaymentReminderResponse>> {
    ensure_stock_manager(&user)?;
    if !payment_webhooks_configured(&state) {
        return Err(ApiError::BadRequest(
            "payment reminder webhooks are not configured".into(),
        ));
    }

    let payment_link = selected_payment_link(&state, &input.payment_link_user_id).await?;
    let entries = payment_reminder_entries(&state, &payment_link).await?;
    if entries.is_empty() {
        return Err(ApiError::BadRequest(
            "no users have open payment balance".into(),
        ));
    }

    let message = payment_reminder_message(&entries);
    notify_payment_reminder(&state, &message).await;

    Ok(Json(PaymentReminderResponse {
        reminded_count: entries.len(),
    }))
}

#[utoipa::path(
    post,
    path = "/api/v1/payments/email-reminders",
    request_body = EmailPaymentReminderRequest,
    responses((status = 200, body = EmailPaymentReminderResponse))
)]
pub(crate) async fn send_email_payment_reminder(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(input): Json<EmailPaymentReminderRequest>,
) -> ApiResult<Json<EmailPaymentReminderResponse>> {
    ensure_stock_manager(&user)?;
    let smtp = state.config.smtp.as_ref().ok_or_else(|| {
        ApiError::ServiceUnavailable("email payment reminders are not configured".into())
    })?;

    let payment_link = selected_payment_link(&state, &input.payment_link_user_id).await?;
    let entries = payment_reminder_entries(&state, &payment_link).await?;
    if entries.is_empty() {
        return Err(ApiError::BadRequest(
            "no users have open payment balance".into(),
        ));
    }

    let (selected_entries, skipped_no_email_count) =
        select_email_reminder_entries(entries, input.recipient_user_ids)?;
    let reminders: Vec<PaymentReminderEmail> = selected_entries
        .into_iter()
        .map(|entry| PaymentReminderEmail {
            user_id: entry.user_id,
            display_name: entry.display_name,
            email: entry.email.expect("selected reminder email was validated"),
            amount_cents: entry.amount_cents,
            payment_link: entry.payment_link,
        })
        .collect();

    let delivery = send_payment_reminder_emails(
        smtp,
        &state.config.app_name,
        &state.config.frontend_base_url,
        &reminders,
    )
    .await?;
    Ok(Json(EmailPaymentReminderResponse {
        sent_count: delivery.sent_count,
        failed_user_ids: delivery.failed_user_ids,
        skipped_no_email_count,
    }))
}

#[utoipa::path(
    post,
    path = "/api/v1/payments",
    request_body = MarkPaidRequest,
    responses((status = 201, body = PaymentRecord))
)]
pub(crate) async fn mark_paid(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(input): Json<MarkPaidRequest>,
) -> ApiResult<(StatusCode, Json<PaymentRecord>)> {
    ensure_can_mark_paid(&user, &input.user_id)?;

    let target_user = load_current_user(&state, &input.user_id).await?;
    let recipient = load_current_user(&state, &input.received_by_user_id).await?;
    ensure_payment_recipient(&recipient)?;

    let totals = user_totals(&state, &input.user_id).await?;
    if totals.unpaid_drink_count == 0 {
        return Err(ApiError::BadRequest(
            "user has no unpaid consumptions".into(),
        ));
    }

    let now = now_ts();
    let id = Uuid::new_v4().to_string();
    let period_end_ts = now;
    sqlx::query(
        "INSERT INTO payments
         (id, user_id, received_by_user_id, amount_cents, drink_count, period_end_ts,
          note, paid_at, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&input.user_id)
    .bind(&input.received_by_user_id)
    .bind(totals.unpaid_amount_cents)
    .bind(totals.unpaid_drink_count)
    .bind(period_end_ts)
    .bind(input.note.as_ref())
    .bind(now)
    .bind(now)
    .execute(&state.db)
    .await?;

    notify_payment_marked_paid(
        &state,
        &target_user.display_name,
        totals.unpaid_amount_cents,
    )
    .await;
    let unlocks =
        evaluate_after_payment(&state, &input.user_id, totals.unpaid_amount_cents, now).await?;
    notify_unlocks(&state, &target_user, &unlocks).await;

    Ok((
        StatusCode::CREATED,
        Json(PaymentRecord {
            id,
            user_id: input.user_id,
            received_by_user_id: Some(input.received_by_user_id),
            amount_cents: totals.unpaid_amount_cents,
            drink_count: totals.unpaid_drink_count,
            period_end_ts,
            note: input.note,
            paid_at: now,
        }),
    ))
}

fn ensure_stock_manager(user: &CurrentUser) -> ApiResult<()> {
    if user.stock_manager {
        Ok(())
    } else {
        Err(ApiError::Forbidden)
    }
}

async fn load_current_user(state: &AppState, user_id: &str) -> ApiResult<CurrentUser> {
    let row: Option<UserProfileRow> = sqlx::query_as(
        "SELECT id, user_type, gitlab_id, display_name, leaderboard_opt_in,
                achievement_sharing_opt_out, mattermost_opt_in, stock_manager, paypal_link
         FROM users
         WHERE id = ?",
    )
    .bind(user_id)
    .fetch_optional(&state.db)
    .await?;
    let row = row.ok_or(ApiError::NotFound)?;
    Ok(CurrentUser {
        id: row.id,
        user_type: row.user_type,
        gitlab_id: row.gitlab_id,
        display_name: row.display_name,
        leaderboard_opt_in: row.leaderboard_opt_in != 0,
        achievement_sharing_opt_out: row.achievement_sharing_opt_out != 0,
        mattermost_opt_in: row.mattermost_opt_in != 0,
        stock_manager: row.stock_manager != 0,
        paypal_link: row.paypal_link,
    })
}

fn ensure_can_mark_paid(user: &CurrentUser, target_user_id: &str) -> ApiResult<()> {
    if user.stock_manager || user.id == target_user_id {
        Ok(())
    } else {
        Err(ApiError::Forbidden)
    }
}

fn ensure_payment_recipient(user: &CurrentUser) -> ApiResult<()> {
    if user.stock_manager {
        Ok(())
    } else {
        Err(ApiError::BadRequest(
            "payment recipient must be a staff member".into(),
        ))
    }
}

async fn received_payment_entries(
    state: &AppState,
    recipient_user_id: &str,
) -> ApiResult<Vec<ReceivedPaymentEntry>> {
    sqlx::query_as::<_, ReceivedPaymentRow>(
        "SELECT p.id,
                p.user_id AS payer_user_id,
                payer.display_name AS payer_display_name,
                p.received_by_user_id AS recipient_user_id,
                recipient.display_name AS recipient_display_name,
                p.amount_cents,
                p.drink_count,
                p.paid_at,
                p.note
         FROM payments p
         JOIN users payer ON payer.id = p.user_id
         JOIN users recipient ON recipient.id = p.received_by_user_id
         WHERE p.received_by_user_id = ?
         ORDER BY p.paid_at DESC, p.created_at DESC",
    )
    .bind(recipient_user_id)
    .fetch_all(&state.db)
    .await?
    .into_iter()
    .map(|row| {
        Ok(ReceivedPaymentEntry {
            id: row.id,
            payer_user_id: row.payer_user_id,
            payer_display_name: row.payer_display_name,
            recipient_user_id: row.recipient_user_id,
            recipient_display_name: row.recipient_display_name,
            amount_cents: row.amount_cents,
            drink_count: row.drink_count,
            paid_at: row.paid_at,
            note: row.note,
        })
    })
    .collect()
}

async fn selected_payment_link(state: &AppState, user_id: &str) -> ApiResult<String> {
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT paypal_link
         FROM users
         WHERE id = ?
           AND stock_manager = 1
           AND paypal_link IS NOT NULL
           AND TRIM(paypal_link) <> ''",
    )
    .bind(user_id)
    .fetch_optional(&state.db)
    .await?;
    row.map(|(paypal_link,)| paypal_link)
        .ok_or_else(|| ApiError::BadRequest("selected payment link is not configured".into()))
}

struct PaymentReminderEntry {
    user_id: String,
    display_name: String,
    email: Option<String>,
    amount_cents: i64,
    payment_link: String,
}

async fn payment_reminder_entries(
    state: &AppState,
    payment_link: &str,
) -> ApiResult<Vec<PaymentReminderEntry>> {
    let users: Vec<UserRow> = sqlx::query_as(
        "SELECT id, display_name, user_type, email FROM users ORDER BY display_name ASC",
    )
    .fetch_all(&state.db)
    .await?;
    let mut entries = Vec::new();
    for user in users {
        let totals = user_totals(state, &user.id).await?;
        let expense_balance_cents = user_expense_balance_cents(state, &user.id).await?;
        let amount_due = amount_due_cents(totals.unpaid_amount_cents, expense_balance_cents);
        if amount_due > 0 {
            entries.push(PaymentReminderEntry {
                user_id: user.id,
                display_name: user.display_name,
                email: user.email,
                amount_cents: amount_due,
                payment_link: build_payment_url(payment_link, amount_due),
            });
        }
    }
    entries.sort_by(|a, b| {
        b.amount_cents
            .cmp(&a.amount_cents)
            .then_with(|| a.display_name.cmp(&b.display_name))
    });
    Ok(entries)
}

fn select_email_reminder_entries(
    entries: Vec<PaymentReminderEntry>,
    recipient_user_ids: Option<Vec<String>>,
) -> ApiResult<(Vec<PaymentReminderEntry>, usize)> {
    match recipient_user_ids {
        Some(recipient_user_ids) => {
            let selected_ids: HashSet<String> = recipient_user_ids.into_iter().collect();
            if selected_ids.is_empty() {
                return Err(ApiError::BadRequest(
                    "at least one email reminder recipient is required".into(),
                ));
            }
            let selected_entries: Vec<_> = entries
                .into_iter()
                .filter(|entry| selected_ids.contains(&entry.user_id))
                .collect();
            if selected_entries.len() != selected_ids.len() {
                return Err(ApiError::BadRequest(
                    "one or more selected users do not have an open payment balance".into(),
                ));
            }
            if selected_entries
                .iter()
                .any(|entry| !email_address_available(entry.email.as_deref()))
            {
                return Err(ApiError::BadRequest(
                    "one or more selected users do not have a valid email address".into(),
                ));
            }
            Ok((selected_entries, 0))
        }
        None => {
            let skipped_no_email_count = entries
                .iter()
                .filter(|entry| !email_address_available(entry.email.as_deref()))
                .count();
            let selected_entries: Vec<_> = entries
                .into_iter()
                .filter(|entry| email_address_available(entry.email.as_deref()))
                .collect();
            if selected_entries.is_empty() {
                return Err(ApiError::BadRequest(
                    "no debtors have a valid email address".into(),
                ));
            }
            Ok((selected_entries, skipped_no_email_count))
        }
    }
}

fn payment_reminder_message(entries: &[PaymentReminderEntry]) -> String {
    let mut message = String::from("Payment reminder:");
    for entry in entries {
        message.push_str(&format!(
            "\n- {}: {} - {}",
            entry.display_name,
            format_euros(entry.amount_cents),
            entry.payment_link
        ));
    }
    message
}

fn build_payment_url(base_link: &str, amount_cents: i64) -> String {
    let amount = format!(
        "{amount_cents_major}.{amount_cents_minor:02}",
        amount_cents_major = amount_cents / 100,
        amount_cents_minor = amount_cents.abs() % 100,
    );
    let trimmed = base_link.trim();
    match url::Url::parse(trimmed) {
        Ok(mut url) => {
            url.set_path(&format!("{}/{}", url.path().trim_end_matches('/'), amount));
            url.to_string()
        }
        Err(_) => format!("{}/{}", trimmed.trim_end_matches('/'), amount),
    }
}

fn format_euros(amount_cents: i64) -> String {
    format!("{}.{:02} EUR", amount_cents / 100, amount_cents.abs() % 100)
}

fn amount_due_cents(unpaid_amount_cents: i64, expense_balance_cents: i64) -> i64 {
    (unpaid_amount_cents - expense_balance_cents).max(0)
}

struct UserTotals {
    unpaid_amount_cents: i64,
    unpaid_drink_count: i64,
    paid_amount_cents: i64,
    paid_drink_count: i64,
    last_paid_at: Option<i64>,
}

async fn user_totals(state: &AppState, user_id: &str) -> ApiResult<UserTotals> {
    let (max_period_end, last_paid_at): (Option<i64>, Option<i64>) =
        sqlx::query_as("SELECT MAX(period_end_ts), MAX(paid_at) FROM payments WHERE user_id = ?")
            .bind(user_id)
            .fetch_one(&state.db)
            .await?;
    let cutoff = max_period_end.unwrap_or(0);

    let unpaid_drink_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)
         FROM consumptions c
         JOIN beverages b ON b.id = c.beverage_id
         WHERE c.user_id = ? AND c.consumed_at >= ?",
    )
    .bind(user_id)
    .bind(cutoff)
    .fetch_one(&state.db)
    .await?;

    // Stamp cards run per calendar month, so positions are counted from the 1st
    // of the month the open tab starts in - not from the payment cutoff itself.
    let tz = resolve_tz(&state.config.timezone);
    let paid_drinks = paid_drinks_since_month_of(state, user_id, cutoff).await?;
    let unpaid_amount_cents = discounted_amount_cents(&paid_drinks, cutoff, &tz);

    let paid: (Option<i64>, Option<i64>) = sqlx::query_as(
        "SELECT SUM(amount_cents), SUM(drink_count) FROM payments WHERE user_id = ?",
    )
    .bind(user_id)
    .fetch_one(&state.db)
    .await?;

    Ok(UserTotals {
        unpaid_amount_cents,
        unpaid_drink_count,
        paid_amount_cents: paid.0.unwrap_or(0),
        paid_drink_count: paid.1.unwrap_or(0),
        last_paid_at,
    })
}

#[derive(sqlx::FromRow)]
struct UserRow {
    id: String,
    display_name: String,
    user_type: String,
    email: Option<String>,
}

#[derive(sqlx::FromRow)]
struct ReceivedPaymentRow {
    id: String,
    payer_user_id: String,
    payer_display_name: String,
    recipient_user_id: String,
    recipient_display_name: String,
    amount_cents: i64,
    drink_count: i64,
    paid_at: i64,
    note: Option<String>,
}

#[derive(sqlx::FromRow)]
struct UserProfileRow {
    id: String,
    user_type: String,
    gitlab_id: Option<i64>,
    display_name: String,
    leaderboard_opt_in: i64,
    achievement_sharing_opt_out: i64,
    mattermost_opt_in: i64,
    stock_manager: i64,
    paypal_link: Option<String>,
}

#[cfg(test)]
mod tests {
    use axum::{Json, extract::State, http::StatusCode};
    use std::{
        io::{Read, Write},
        net::TcpListener,
        sync::mpsc,
        time::Duration,
    };

    use super::*;
    use crate::{config::Config, db::connect};

    fn user(id: &str, user_type: &str) -> CurrentUser {
        CurrentUser {
            id: id.to_string(),
            user_type: user_type.to_string(),
            gitlab_id: (user_type == "member").then_some(1),
            display_name: id.to_string(),
            leaderboard_opt_in: true,
            achievement_sharing_opt_out: false,
            mattermost_opt_in: false,
            stock_manager: id == "actor",
            paypal_link: None,
        }
    }

    async fn state() -> AppState {
        let db_path = format!("/tmp/coffee-payments-{}.db", Uuid::new_v4());
        let db = connect(&format!("sqlite://{db_path}")).await.unwrap();
        sqlx::query(
            "INSERT INTO users
             (id, user_type, gitlab_id, display_name, leaderboard_opt_in,
              achievement_sharing_opt_out, mattermost_opt_in, stock_manager, created_at, updated_at)
             VALUES
             ('actor', 'member', 1, 'Actor', 1, 0, 0, 1, 1, 1),
             ('target', 'member', 2, 'Target', 1, 0, 0, 0, 1, 1),
             ('guest', 'guest', NULL, 'Guest', 0, 0, 0, 0, 1, 1)",
        )
        .execute(&db)
        .await
        .unwrap();
        sqlx::query("INSERT INTO machines (id, name, created_at) VALUES ('main', 'Main', 1)")
            .execute(&db)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO beverages (id, name, price_cents, caffeine_mg, created_at)
             VALUES ('coffee', 'Coffee', 50, 90, 1)",
        )
        .execute(&db)
        .await
        .unwrap();
        AppState {
            config: Config {
                app_name: "Coffee Tracker".into(),
                database_url: format!("sqlite://{db_path}"),
                bind_addr: "127.0.0.1:0".parse().unwrap(),
                session_secret: "test".into(),
                gitlab_base_url: "https://gitlab.example.org".into(),
                gitlab_oauth_client_id: String::new(),
                gitlab_oauth_client_secret: String::new(),
                gitlab_oauth_scopes: "read_user".into(),
                authentik_issuer_url: String::new(),
                authentik_oauth_client_id: String::new(),
                authentik_oauth_client_secret: String::new(),
                authentik_oauth_scopes: "openid email profile groups".into(),
                auth_allowed_groups: Vec::new(),
                auth_staff_groups: Vec::new(),
                public_base_url: "http://localhost".into(),
                frontend_base_url: "http://localhost".into(),
                mattermost_webhook_url: None,
                matrix_webhook_url: None,
                smtp: None,
                paypal_link: None,
                timezone: "UTC".into(),
                session_ttl: std::time::Duration::from_secs(60),
            },
            db,
            http: reqwest::Client::new(),
        }
    }

    async fn insert_consumption(state: &AppState, id: &str, consumed_at: i64) {
        insert_consumption_for_user(state, id, "target", consumed_at).await;
    }

    async fn insert_consumption_for_user(
        state: &AppState,
        id: &str,
        user_id: &str,
        consumed_at: i64,
    ) {
        sqlx::query(
            "INSERT INTO consumptions
             (id, user_id, machine_id, beverage_id, consumed_at, created_at)
             VALUES (?, ?, 'main', 'coffee', ?, ?)",
        )
        .bind(id)
        .bind(user_id)
        .bind(consumed_at)
        .bind(consumed_at)
        .execute(&state.db)
        .await
        .unwrap();
    }

    async fn insert_expense_for_user(
        state: &AppState,
        id: &str,
        user_id: &str,
        amount_cents: i64,
        logged_at: i64,
    ) {
        sqlx::query(
            "INSERT INTO expense_entries
             (id, user_id, created_by_user_id, description, amount_cents, logged_at, created_at)
             VALUES (?, ?, 'actor', 'Ice', ?, ?, ?)",
        )
        .bind(id)
        .bind(user_id)
        .bind(amount_cents)
        .bind(logged_at)
        .bind(logged_at)
        .execute(&state.db)
        .await
        .unwrap();
    }

    fn webhook_probe() -> (String, mpsc::Receiver<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut buffer = [0_u8; 4096];
            let bytes = stream.read(&mut buffer).unwrap_or(0);
            let request = String::from_utf8_lossy(&buffer[..bytes]).to_string();
            let _ = tx.send(request);
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n")
                .unwrap();
        });
        (url, rx)
    }

    #[tokio::test]
    async fn user_totals_split_paid_and_unpaid_consumptions() {
        let state = state().await;
        insert_consumption(&state, "old", 10).await;
        sqlx::query(
            "INSERT INTO payments
             (id, user_id, amount_cents, drink_count, period_end_ts, paid_at, created_at)
             VALUES ('payment', 'target', 50, 1, 20, 20, 20)",
        )
        .execute(&state.db)
        .await
        .unwrap();
        insert_consumption(&state, "new", 30).await;

        let totals = user_totals(&state, "target").await.unwrap();
        assert_eq!(totals.paid_amount_cents, 50);
        assert_eq!(totals.paid_drink_count, 1);
        assert_eq!(totals.unpaid_amount_cents, 50);
        assert_eq!(totals.unpaid_drink_count, 1);
    }

    #[tokio::test]
    async fn user_totals_deducts_the_free_stamp_card_drink() {
        let state = state().await;
        // 13 drinks in the same calendar month: the 13th one is on the house.
        for index in 0..13 {
            insert_consumption(&state, &format!("drink-{index}"), 1_000 + index * 60).await;
        }

        let totals = user_totals(&state, "target").await.unwrap();
        assert_eq!(totals.unpaid_amount_cents, 12 * 50);
        assert_eq!(totals.unpaid_drink_count, 13);
    }

    #[tokio::test]
    async fn free_drink_position_ignores_the_payment_cutoff() {
        let state = state().await;
        for index in 0..13 {
            insert_consumption(&state, &format!("drink-{index}"), 1_000 + index * 60).await;
        }
        // Settle the first eight drinks; positions still count from the 1st of the month.
        sqlx::query(
            "INSERT INTO payments
             (id, user_id, amount_cents, drink_count, period_end_ts, paid_at, created_at)
             VALUES ('payment', 'target', 400, 8, 1480, 1480, 1480)",
        )
        .execute(&state.db)
        .await
        .unwrap();

        let totals = user_totals(&state, "target").await.unwrap();
        assert_eq!(totals.unpaid_drink_count, 5);
        assert_eq!(totals.unpaid_amount_cents, 4 * 50);
    }

    #[tokio::test]
    async fn mark_paid_settles_current_unpaid_tab() {
        let state = state().await;
        insert_consumption(&state, "first", now_ts() - 10).await;
        insert_consumption(&state, "second", now_ts() - 5).await;

        let (status, Json(record)) = mark_paid(
            State(state.clone()),
            user("actor", "member"),
            Json(MarkPaidRequest {
                user_id: "target".into(),
                received_by_user_id: "actor".into(),
                note: None,
            }),
        )
        .await
        .unwrap();

        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(record.amount_cents, 100);
        assert_eq!(record.drink_count, 2);
        let totals = user_totals(&state, "target").await.unwrap();
        assert_eq!(totals.unpaid_amount_cents, 0);
        assert_eq!(totals.paid_amount_cents, 100);
    }

    #[tokio::test]
    async fn guest_self_payment_sends_mattermost_notice() {
        let mut state = state().await;
        let (webhook_url, receiver) = webhook_probe();
        state.config.mattermost_webhook_url = Some(webhook_url);
        insert_consumption_for_user(&state, "guest-cup", "guest", now_ts() - 10).await;

        let (status, Json(record)) = mark_paid(
            State(state),
            user("guest", "guest"),
            Json(MarkPaidRequest {
                user_id: "guest".into(),
                received_by_user_id: "actor".into(),
                note: None,
            }),
        )
        .await
        .unwrap();

        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(record.amount_cents, 50);
        let request = receiver.recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(request.contains("Guest marked their debt of 0.50 EUR as paid."));
    }

    #[tokio::test]
    async fn guest_self_payment_sends_matrix_notice() {
        let mut state = state().await;
        let (webhook_url, receiver) = webhook_probe();
        state.config.matrix_webhook_url = Some(webhook_url);
        insert_consumption_for_user(&state, "guest-cup", "guest", now_ts() - 10).await;

        let (status, Json(record)) = mark_paid(
            State(state),
            user("guest", "guest"),
            Json(MarkPaidRequest {
                user_id: "guest".into(),
                received_by_user_id: "actor".into(),
                note: None,
            }),
        )
        .await
        .unwrap();

        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(record.amount_cents, 50);
        let request = receiver.recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(request.contains("Guest marked their debt of 0.50 EUR as paid."));
    }

    #[tokio::test]
    async fn guest_self_payment_sends_all_configured_webhooks() {
        let mut state = state().await;
        let (mattermost_webhook_url, mattermost_receiver) = webhook_probe();
        let (matrix_webhook_url, matrix_receiver) = webhook_probe();
        state.config.mattermost_webhook_url = Some(mattermost_webhook_url);
        state.config.matrix_webhook_url = Some(matrix_webhook_url);
        insert_consumption_for_user(&state, "guest-cup", "guest", now_ts() - 10).await;

        let (status, Json(record)) = mark_paid(
            State(state),
            user("guest", "guest"),
            Json(MarkPaidRequest {
                user_id: "guest".into(),
                received_by_user_id: "actor".into(),
                note: None,
            }),
        )
        .await
        .unwrap();

        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(record.amount_cents, 50);
        let mattermost_request = mattermost_receiver
            .recv_timeout(Duration::from_secs(2))
            .unwrap();
        let matrix_request = matrix_receiver
            .recv_timeout(Duration::from_secs(2))
            .unwrap();
        assert!(mattermost_request.contains("Guest marked their debt of 0.50 EUR as paid."));
        assert!(matrix_request.contains("Guest marked their debt of 0.50 EUR as paid."));
    }

    #[tokio::test]
    async fn only_stock_managers_can_view_shared_payments() {
        let state = state().await;
        let result = list_payments(State(state.clone()), user("guest", "guest")).await;
        assert!(matches!(result, Err(ApiError::Forbidden)));

        let result = list_payments(State(state.clone()), user("target", "member")).await;
        assert!(matches!(result, Err(ApiError::Forbidden)));

        let result = list_payments(State(state), user("actor", "member")).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn authenticated_users_can_list_configured_staff_payment_links() {
        let state = state().await;
        sqlx::query(
            "UPDATE users SET paypal_link = 'https://paypal.example/pay' WHERE id = 'actor'",
        )
        .execute(&state.db)
        .await
        .unwrap();
        sqlx::query(
            "UPDATE users SET paypal_link = 'https://paypal.example/ignored' WHERE id = 'target'",
        )
        .execute(&state.db)
        .await
        .unwrap();

        let Json(response) = list_paypal_links(State(state), user("guest", "guest"))
            .await
            .unwrap();

        assert_eq!(response.entries.len(), 1);
        assert_eq!(response.entries[0].user_id, "actor");
        assert_eq!(
            response.entries[0].paypal_link,
            "https://paypal.example/pay"
        );
    }

    #[tokio::test]
    async fn authenticated_users_can_list_all_staff_payment_recipients() {
        let state = state().await;
        sqlx::query(
            "UPDATE users SET paypal_link = 'https://paypal.example/pay' WHERE id = 'actor'",
        )
        .execute(&state.db)
        .await
        .unwrap();
        sqlx::query("UPDATE users SET stock_manager = 1 WHERE id = 'target'")
            .execute(&state.db)
            .await
            .unwrap();

        let Json(response) = list_payment_recipients(State(state), user("guest", "guest"))
            .await
            .unwrap();

        assert_eq!(response.entries.len(), 2);
        assert_eq!(response.entries[0].user_id, "actor");
        assert_eq!(
            response.entries[0].paypal_link.as_deref(),
            Some("https://paypal.example/pay")
        );
        assert_eq!(response.entries[1].user_id, "target");
        assert_eq!(response.entries[1].paypal_link, None);
    }

    #[tokio::test]
    async fn mark_paid_reduces_recipient_expense_balance() {
        let state = state().await;
        insert_consumption(&state, "target-cup", now_ts() - 10).await;
        sqlx::query(
            "INSERT INTO expense_entries
             (id, user_id, created_by_user_id, description, amount_cents, logged_at, created_at)
             VALUES ('expense', 'actor', 'actor', 'Coffee beans', 5000, 1, 1)",
        )
        .execute(&state.db)
        .await
        .unwrap();

        let (status, Json(record)) = mark_paid(
            State(state.clone()),
            user("target", "member"),
            Json(MarkPaidRequest {
                user_id: "target".into(),
                received_by_user_id: "actor".into(),
                note: None,
            }),
        )
        .await
        .unwrap();

        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(record.received_by_user_id.as_deref(), Some("actor"));
        assert_eq!(
            user_expense_total_cents(&state, "actor").await.unwrap(),
            5000
        );
        assert_eq!(
            user_expense_balance_cents(&state, "actor").await.unwrap(),
            4950
        );
        let totals = user_totals(&state, "target").await.unwrap();
        assert_eq!(totals.unpaid_amount_cents, 0);
        sqlx::query("UPDATE users SET stock_manager = 1 WHERE id = 'target'")
            .execute(&state.db)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO payments
             (id, user_id, received_by_user_id, amount_cents, drink_count, period_end_ts,
              paid_at, created_at)
             VALUES ('other-recipient-payment', 'guest', 'target', 100, 2, 30, 30, 30)",
        )
        .execute(&state.db)
        .await
        .unwrap();

        let Json(response) = list_payments(State(state), user("actor", "member"))
            .await
            .unwrap();
        let actor_summary = response
            .entries
            .iter()
            .find(|entry| entry.user_id == "actor")
            .unwrap();
        assert_eq!(actor_summary.expense_amount_cents, 5000);
        assert_eq!(actor_summary.received_payment_amount_cents, 50);
        assert_eq!(actor_summary.expense_balance_cents, 4950);
        assert_eq!(response.received_payments.len(), 1);
        assert_eq!(response.received_payments[0].payer_user_id, "target");
        assert_eq!(response.received_payments[0].payer_display_name, "Target");
        assert_eq!(response.received_payments[0].recipient_user_id, "actor");
        assert_eq!(
            response.received_payments[0].recipient_display_name,
            "Actor"
        );
        assert_eq!(response.received_payments[0].amount_cents, 50);
    }

    #[tokio::test]
    async fn mark_paid_to_self_settles_debt_and_reduces_own_expense_balance() {
        let state = state().await;
        for index in 0..13 {
            insert_consumption_for_user(
                &state,
                &format!("actor-cup-{index}"),
                "actor",
                now_ts() - 20 - index,
            )
            .await;
        }
        sqlx::query(
            "INSERT INTO expense_entries
             (id, user_id, created_by_user_id, description, amount_cents, logged_at, created_at)
             VALUES ('actor-expense', 'actor', 'actor', 'Coffee beans', 1000, 1, 1)",
        )
        .execute(&state.db)
        .await
        .unwrap();

        let (status, Json(record)) = mark_paid(
            State(state.clone()),
            user("actor", "member"),
            Json(MarkPaidRequest {
                user_id: "actor".into(),
                received_by_user_id: "actor".into(),
                note: None,
            }),
        )
        .await
        .unwrap();

        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(record.user_id, "actor");
        assert_eq!(record.received_by_user_id.as_deref(), Some("actor"));
        // 13 drinks in one month: the 13th is free thanks to the stamp card.
        assert_eq!(record.amount_cents, 600);
        assert_eq!(record.drink_count, 13);

        let totals = user_totals(&state, "actor").await.unwrap();
        assert_eq!(totals.unpaid_amount_cents, 0);
        assert_eq!(totals.paid_amount_cents, 600);
        assert_eq!(
            user_expense_total_cents(&state, "actor").await.unwrap(),
            1000
        );
        assert_eq!(
            user_expense_balance_cents(&state, "actor").await.unwrap(),
            400
        );
    }

    #[tokio::test]
    async fn mark_paid_requires_staff_recipient() {
        let state = state().await;
        insert_consumption(&state, "target-cup", now_ts() - 10).await;

        let result = mark_paid(
            State(state),
            user("target", "member"),
            Json(MarkPaidRequest {
                user_id: "target".into(),
                received_by_user_id: "target".into(),
                note: None,
            }),
        )
        .await;

        assert!(matches!(result, Err(ApiError::BadRequest(_))));
    }

    #[tokio::test]
    async fn only_stock_managers_can_send_payment_reminders() {
        let mut state = state().await;
        let (webhook_url, _receiver) = webhook_probe();
        state.config.mattermost_webhook_url = Some(webhook_url);
        sqlx::query(
            "UPDATE users SET paypal_link = 'https://paypal.example/pay' WHERE id = 'actor'",
        )
        .execute(&state.db)
        .await
        .unwrap();
        insert_consumption(&state, "target-cup", now_ts() - 10).await;

        let result = send_payment_reminder(
            State(state),
            user("target", "member"),
            Json(PaymentReminderRequest {
                payment_link_user_id: "actor".into(),
            }),
        )
        .await;

        assert!(matches!(result, Err(ApiError::Forbidden)));
    }

    #[tokio::test]
    async fn email_payment_reminders_require_staff_and_smtp_configuration() {
        let state = state().await;
        let request = || {
            Json(EmailPaymentReminderRequest {
                payment_link_user_id: "actor".into(),
                recipient_user_ids: None,
            })
        };

        let non_staff =
            send_email_payment_reminder(State(state.clone()), user("target", "member"), request())
                .await;
        assert!(matches!(non_staff, Err(ApiError::Forbidden)));

        let unconfigured =
            send_email_payment_reminder(State(state), user("actor", "member"), request()).await;
        assert!(matches!(unconfigured, Err(ApiError::ServiceUnavailable(_))));
    }

    #[tokio::test]
    async fn shared_payment_entries_only_expose_email_availability() {
        let state = state().await;
        sqlx::query("UPDATE users SET email = 'target@example.org' WHERE id = 'target'")
            .execute(&state.db)
            .await
            .unwrap();
        sqlx::query("UPDATE users SET email = 'invalid' WHERE id = 'guest'")
            .execute(&state.db)
            .await
            .unwrap();

        let Json(response) = list_payments(State(state), user("actor", "member"))
            .await
            .unwrap();
        let target = response
            .entries
            .iter()
            .find(|entry| entry.user_id == "target")
            .unwrap();
        let guest = response
            .entries
            .iter()
            .find(|entry| entry.user_id == "guest")
            .unwrap();
        assert!(target.email_available);
        assert!(!guest.email_available);
    }

    #[tokio::test]
    async fn payment_reminder_requires_configured_staff_payment_link() {
        let mut state = state().await;
        let (webhook_url, _receiver) = webhook_probe();
        state.config.mattermost_webhook_url = Some(webhook_url);
        insert_consumption(&state, "target-cup", now_ts() - 10).await;

        let result = send_payment_reminder(
            State(state),
            user("actor", "member"),
            Json(PaymentReminderRequest {
                payment_link_user_id: "target".into(),
            }),
        )
        .await;

        assert!(matches!(result, Err(ApiError::BadRequest(_))));
    }

    #[tokio::test]
    async fn payment_reminder_requires_configured_webhooks() {
        let state = state().await;
        sqlx::query(
            "UPDATE users SET paypal_link = 'https://paypal.example/pay' WHERE id = 'actor'",
        )
        .execute(&state.db)
        .await
        .unwrap();
        insert_consumption(&state, "target-cup", now_ts() - 10).await;

        let result = send_payment_reminder(
            State(state),
            user("actor", "member"),
            Json(PaymentReminderRequest {
                payment_link_user_id: "actor".into(),
            }),
        )
        .await;

        assert!(matches!(result, Err(ApiError::BadRequest(_))));
    }

    #[tokio::test]
    async fn payment_reminder_requires_open_debt() {
        let mut state = state().await;
        let (webhook_url, _receiver) = webhook_probe();
        state.config.mattermost_webhook_url = Some(webhook_url);
        sqlx::query(
            "UPDATE users SET paypal_link = 'https://paypal.example/pay' WHERE id = 'actor'",
        )
        .execute(&state.db)
        .await
        .unwrap();

        let result = send_payment_reminder(
            State(state),
            user("actor", "member"),
            Json(PaymentReminderRequest {
                payment_link_user_id: "actor".into(),
            }),
        )
        .await;

        assert!(matches!(result, Err(ApiError::BadRequest(_))));
    }

    #[tokio::test]
    async fn payment_reminder_uses_net_amount_due_after_expenses() {
        let state = state().await;
        let now = now_ts();
        for index in 0..26 {
            insert_consumption(&state, &format!("target-cup-{index}"), now - 100 - index).await;
        }
        insert_expense_for_user(&state, "target-ice", "target", 800, now - 50).await;
        insert_consumption_for_user(&state, "actor-cup-1", "actor", now - 20).await;
        insert_consumption_for_user(&state, "actor-cup-2", "actor", now - 19).await;
        insert_expense_for_user(&state, "actor-ice", "actor", 100, now - 10).await;

        let entries = payment_reminder_entries(&state, "https://paypal.example/pay")
            .await
            .unwrap();

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].display_name, "Target");
        // 26 drinks give two free ones, so 24 * 50 cents minus the 800 cents expense.
        assert_eq!(entries[0].amount_cents, 400);
        assert_eq!(entries[0].payment_link, "https://paypal.example/pay/4.00");
    }

    #[tokio::test]
    async fn payment_reminder_posts_open_debts_to_all_webhooks() {
        let mut state = state().await;
        let (mattermost_webhook_url, mattermost_receiver) = webhook_probe();
        let (matrix_webhook_url, matrix_receiver) = webhook_probe();
        state.config.mattermost_webhook_url = Some(mattermost_webhook_url);
        state.config.matrix_webhook_url = Some(matrix_webhook_url);
        sqlx::query(
            "UPDATE users SET paypal_link = 'https://paypal.example/pay' WHERE id = 'actor'",
        )
        .execute(&state.db)
        .await
        .unwrap();
        insert_consumption(&state, "target-cup-1", now_ts() - 10).await;
        insert_consumption(&state, "target-cup-2", now_ts() - 9).await;
        insert_consumption_for_user(&state, "guest-cup", "guest", now_ts() - 8).await;

        let Json(response) = send_payment_reminder(
            State(state),
            user("actor", "member"),
            Json(PaymentReminderRequest {
                payment_link_user_id: "actor".into(),
            }),
        )
        .await
        .unwrap();

        assert_eq!(response.reminded_count, 2);
        let mattermost_request = mattermost_receiver
            .recv_timeout(Duration::from_secs(2))
            .unwrap();
        let matrix_request = matrix_receiver
            .recv_timeout(Duration::from_secs(2))
            .unwrap();
        for request in [mattermost_request, matrix_request] {
            assert!(request.contains("Payment reminder:"));
            assert!(request.contains("- Target: 1.00 EUR - https://paypal.example/pay/1.00"));
            assert!(request.contains("- Guest: 0.50 EUR - https://paypal.example/pay/0.50"));
            assert!(!request.contains("- Actor:"));
        }
    }

    #[test]
    fn amount_due_is_debt_minus_expense_balance_flooring_at_zero() {
        assert_eq!(amount_due_cents(1300, 800), 500);
        assert_eq!(amount_due_cents(1300, 1500), 0);
        assert_eq!(amount_due_cents(1300, -200), 1500);
    }

    #[test]
    fn payment_reminder_message_is_markdown_bullet_list() {
        let message = payment_reminder_message(&[
            PaymentReminderEntry {
                user_id: "target".into(),
                display_name: "Target".into(),
                email: Some("target@example.org".into()),
                amount_cents: 100,
                payment_link: "https://paypal.example/pay/1.00".into(),
            },
            PaymentReminderEntry {
                user_id: "guest".into(),
                display_name: "Guest".into(),
                email: None,
                amount_cents: 50,
                payment_link: "https://paypal.example/pay/0.50".into(),
            },
        ]);

        assert_eq!(
            message,
            "Payment reminder:\n- Target: 1.00 EUR - https://paypal.example/pay/1.00\n- Guest: 0.50 EUR - https://paypal.example/pay/0.50"
        );
    }

    #[test]
    fn all_email_reminders_skip_debtors_without_valid_email() {
        let entries = vec![
            PaymentReminderEntry {
                user_id: "target".into(),
                display_name: "Target".into(),
                email: Some("target@example.org".into()),
                amount_cents: 100,
                payment_link: "https://paypal.example/pay/1.00".into(),
            },
            PaymentReminderEntry {
                user_id: "guest".into(),
                display_name: "Guest".into(),
                email: None,
                amount_cents: 50,
                payment_link: "https://paypal.example/pay/0.50".into(),
            },
        ];

        let (selected, skipped) = select_email_reminder_entries(entries, None).unwrap();
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].user_id, "target");
        assert_eq!(skipped, 1);
    }

    #[test]
    fn custom_email_reminder_selection_deduplicates_and_validates_users() {
        let entries = || {
            vec![
                PaymentReminderEntry {
                    user_id: "target".into(),
                    display_name: "Target".into(),
                    email: Some("target@example.org".into()),
                    amount_cents: 100,
                    payment_link: "https://paypal.example/pay/1.00".into(),
                },
                PaymentReminderEntry {
                    user_id: "guest".into(),
                    display_name: "Guest".into(),
                    email: None,
                    amount_cents: 50,
                    payment_link: "https://paypal.example/pay/0.50".into(),
                },
            ]
        };

        let (selected, skipped) =
            select_email_reminder_entries(entries(), Some(vec!["target".into(), "target".into()]))
                .unwrap();
        assert_eq!(selected.len(), 1);
        assert_eq!(skipped, 0);

        assert!(select_email_reminder_entries(entries(), Some(vec!["guest".into()])).is_err());
        assert!(select_email_reminder_entries(entries(), Some(vec!["unknown".into()])).is_err());
    }

    #[tokio::test]
    async fn non_staff_can_mark_only_their_own_tab_paid() {
        let state = state().await;
        insert_consumption(&state, "target-cup", now_ts() - 10).await;

        let result = mark_paid(
            State(state.clone()),
            user("target", "member"),
            Json(MarkPaidRequest {
                user_id: "target".into(),
                received_by_user_id: "actor".into(),
                note: None,
            }),
        )
        .await;
        assert!(result.is_ok());

        insert_consumption(&state, "target-cup-2", now_ts() - 1).await;
        let result = mark_paid(
            State(state),
            user("guest", "guest"),
            Json(MarkPaidRequest {
                user_id: "target".into(),
                received_by_user_id: "actor".into(),
                note: None,
            }),
        )
        .await;
        assert!(matches!(result, Err(ApiError::Forbidden)));
    }
}
