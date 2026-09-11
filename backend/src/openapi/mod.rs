use axum::Router;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::{
    AppState, achievements, auth, catalog, consumptions, expenses, loyalty, payments, social, stats,
    stock, version,
};

pub fn router() -> Router<AppState> {
    SwaggerUi::new("/swagger-ui")
        .url("/api-docs/openapi.json", ApiDoc::openapi())
        .into()
}

pub fn openapi_json() -> String {
    serde_json::to_string(&ApiDoc::openapi()).expect("openapi spec serializes")
}

#[derive(OpenApi)]
#[openapi(
    paths(
        auth::gitlab_login,
        auth::gitlab_callback,
        auth::authentik_login,
        auth::authentik_callback,
        auth::guest_login,
        auth::logout,
        auth::me,
        auth::reset_guest_recovery_code,
        auth::update_settings,
        catalog::list_machines,
        catalog::list_beverages,
        consumptions::create_consumption,
        consumptions::delete_consumption,
        stats::summary,
        stats::buckets,
        stats::hourly,
        stats::my_beverages,
        stats::streaks,
        stats::caffeine,
        stats::global_summary,
        stats::global_hourly,
        stats::global_buckets,
        stats::global_beverages,
        achievements::my_achievements,
        loyalty::my_stamp_card,
        social::leaderboard,
        social::coffee_twins,
        payments::list_payments,
        payments::my_payments,
        payments::mark_paid,
        payments::send_payment_reminder,
        payments::send_email_payment_reminder,
        payments::payment_config,
        payments::list_paypal_links,
        payments::list_payment_recipients,
        expenses::list_expenses,
        expenses::create_expense,
        stock::list_stock,
        stock::update_stock,
        version::version
    ),
    components(
        schemas(
            auth::CurrentUser,
            auth::MeResponse,
            auth::GuestLoginRequest,
            auth::UpdateSettingsRequest,
            catalog::Machine,
            catalog::Beverage,
            catalog::MachinesResponse,
            catalog::BeveragesResponse,
            consumptions::CreateConsumptionRequest,
            consumptions::ConsumptionResponse,
            stats::BeverageSummary,
            stats::RecentConsumption,
            stats::SummaryResponse,
            stats::Bucket,
            stats::BucketsResponse,
            stats::HourBucket,
            stats::HourlyResponse,
            stats::MyBeverageResponse,
            stats::StreaksResponse,
            stats::CaffeineResponse,
            stats::GlobalTopDrinker,
            stats::GlobalStatsResponse,
            stats::GlobalHourBucket,
            stats::GlobalHourlyResponse,
            stats::GlobalBucket,
            stats::GlobalBucketsResponse,
            stats::GlobalBeverageResponse,
            loyalty::Stamp,
            loyalty::StampCard,
            loyalty::StampCardResponse,
            achievements::AchievementItem,
            achievements::AchievementsResponse,
            social::LeaderboardEntry,
            social::StreakLeaderboardEntry,
            social::AchievementLeaderboardEntry,
            social::AchievementUnlockSummary,
            social::RareAchievement,
            social::RecentAchievementUnlock,
            social::LeaderboardSummary,
            social::LeaderboardResponse,
            social::CoffeeTwin,
            social::CoffeeTwinsResponse,
            payments::PaymentSummary,
            payments::PaymentsResponse,
            payments::MyPaymentResponse,
            payments::PaymentConfigResponse,
            payments::PaypalLinkEntry,
            payments::PaypalLinksResponse,
            payments::PaymentRecipient,
            payments::PaymentRecipientsResponse,
            payments::MarkPaidRequest,
            payments::PaymentReminderRequest,
            payments::PaymentReminderResponse,
            payments::EmailPaymentReminderRequest,
            payments::EmailPaymentReminderResponse,
            payments::PaymentRecord,
            payments::ReceivedPaymentEntry,
            expenses::ExpenseEntry,
            expenses::ExpenseSummary,
            expenses::ExpensesResponse,
            expenses::CreateExpenseRequest,
            stock::StockItem,
            stock::StockResponse,
            stock::UpdateStockRequest,
            version::VersionResponse
        )
    ),
    tags(
        (name = "coffee-tracker", description = "Coffee Tracker backend API")
    )
)]
struct ApiDoc;
