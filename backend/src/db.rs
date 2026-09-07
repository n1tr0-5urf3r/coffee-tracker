use std::str::FromStr;

use chrono::Utc;
use sqlx::{SqlitePool, sqlite::SqlitePoolOptions};

pub fn now_ts() -> i64 {
    Utc::now().timestamp()
}

pub async fn connect(database_url: &str) -> anyhow::Result<SqlitePool> {
    let options = sqlx::sqlite::SqliteConnectOptions::from_str(database_url)?
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?;
    let mut migrator = sqlx::migrate!("./migrations");
    // A retired deployment-specific data repair used this migration sequence.
    // Existing databases may still record it even though fresh installations
    // should not receive that private data migration.
    migrator.set_ignore_missing(true).run(&pool).await?;
    Ok(pool)
}

pub async fn seed_defaults(pool: &SqlitePool) -> anyhow::Result<()> {
    let now = now_ts();

    sqlx::query(
        "INSERT OR IGNORE INTO machines (id, name, location, created_at) VALUES (?, ?, ?, ?)",
    )
    .bind("main")
    .bind("Coffee machine")
    .bind("Common area")
    .bind(now)
    .execute(pool)
    .await?;

    sqlx::query(
        "INSERT OR IGNORE INTO beverages (id, name, price_cents, caffeine_mg, created_at)
         VALUES (?, ?, ?, ?, ?), (?, ?, ?, ?, ?), (?, ?, ?, ?, ?), (?, ?, ?, ?, ?), (?, ?, ?, ?, ?)",
    )
    .bind("coffee")
    .bind("Coffee")
    .bind(50_i64)
    .bind(90_i64)
    .bind(now)
    .bind("espresso_pads")
    .bind("Espresso (pads)")
    .bind(50_i64)
    .bind(90_i64)
    .bind(now)
    .bind("eis")
    .bind("Eis")
    .bind(50_i64)
    .bind(0_i64)
    .bind(now)
    .bind("tea")
    .bind("Tea")
    .bind(25_i64)
    .bind(0_i64)
    .bind(now)
    .bind("other")
    .bind("Other")
    .bind(100_i64)
    .bind(0_i64)
    .bind(now)
    .execute(pool)
    .await?;

    seed_achievements(pool, now).await?;
    Ok(())
}

async fn seed_achievements(pool: &SqlitePool, now: i64) -> anyhow::Result<()> {
    let definitions = [
        (
            "first_coffee",
            "First coffee",
            "Logged the first coffee.",
            "Log one coffee.",
            "lifetime_cups",
            1,
        ),
        (
            "early_bird",
            "Early bird",
            "Logged coffee before 08:00.",
            "Start the day early.",
            "early_bird",
            1,
        ),
        (
            "night_owl",
            "Night owl",
            "Logged coffee at or after 22:00.",
            "Have a late lab session.",
            "night_owl",
            1,
        ),
        (
            "century",
            "Century",
            "Logged 100 coffees.",
            "Reach 100 coffees.",
            "lifetime_cups",
            100,
        ),
        (
            "half_century",
            "Half century",
            "Logged 50 coffees.",
            "Reach 50 coffees.",
            "lifetime_cups",
            50,
        ),
        (
            "millennium",
            "Millennium",
            "Logged 1000 coffees.",
            "Reach 1000 coffees.",
            "lifetime_cups",
            1000,
        ),
        (
            "week_streak",
            "Week streak",
            "Logged coffee on seven consecutive days.",
            "Keep a seven-day streak.",
            "streak_days",
            7,
        ),
        (
            "three_day_streak",
            "Steady start",
            "Logged drinks on three consecutive lab days.",
            "Keep a three-day streak.",
            "streak_days",
            3,
        ),
        (
            "variety",
            "Variety",
            "Logged both beverage types.",
            "Try every beverage category.",
            "beverage_variety",
            2,
        ),
        (
            "marathon_day",
            "Marathon day",
            "Logged five drinks in one day.",
            "Log five drinks in one day.",
            "single_day_count",
            5,
        ),
        (
            "triple_day",
            "Triple shot",
            "Logged three drinks in one day.",
            "Log three drinks in one day.",
            "single_day_count",
            3,
        ),
        (
            "morning_regular",
            "Morning regular",
            "Logged 10 drinks before 10:00.",
            "Log 10 drinks before 10:00.",
            "hour_window_count:0:10",
            10,
        ),
        (
            "lunch_break",
            "Lunch break",
            "Logged five drinks between 12:00 and 14:00.",
            "Log five drinks during lunch.",
            "hour_window_count:12:14",
            5,
        ),
        (
            "weekend_visit",
            "Weekend visit",
            "Logged a weekend drink.",
            "Log a drink on Saturday or Sunday.",
            "weekend_count",
            1,
        ),
        (
            "welcome_back",
            "Welcome back",
            "Logged a drink after at least 14 days away.",
            "Return after a 14-day break.",
            "return_after_gap_days",
            14,
        ),
        (
            "caffeine_high",
            "Caffeine high",
            "Crossed 400mg estimated caffeine in bloodstream.",
            "Cross the 400mg caffeine threshold.",
            "caffeine_threshold",
            400,
        ),
        (
            "debt_marker_5",
            "Five euro tab",
            "Reached 5 EUR in current unpaid coffee debt.",
            "Let the current tab reach 5 EUR.",
            "unpaid_amount",
            500,
        ),
        (
            "debt_marker_10",
            "Ten euro tab",
            "Reached 10 EUR in current unpaid coffee debt.",
            "Let the current tab reach 10 EUR.",
            "unpaid_amount",
            1000,
        ),
        (
            "first_payment",
            "First payment",
            "Marked the first coffee payment as paid.",
            "Settle your first coffee tab.",
            "payment_count",
            1,
        ),
        (
            "third_payment",
            "Reliable payer",
            "Marked three coffee payments as paid.",
            "Settle three coffee tabs.",
            "payment_count",
            3,
        ),
        (
            "big_settlement",
            "Big settlement",
            "Paid off at least 5 EUR in one payment.",
            "Settle a tab of at least 5 EUR.",
            "single_payment_amount",
            500,
        ),
        (
            "first_espresso_pads",
            "Pad pilot",
            "Logged the first Espresso (pads).",
            "Try an Espresso from pads.",
            "beverage_count:espresso_pads",
            1,
        ),
        (
            "espresso_pads_regular",
            "Pad regular",
            "Logged 10 Espresso (pads).",
            "Log 10 Espresso pads.",
            "beverage_count:espresso_pads",
            10,
        ),
        (
            "espresso_pads_fan",
            "Pad fan",
            "Logged 25 Espresso (pads).",
            "Log 25 Espresso pads.",
            "beverage_count:espresso_pads",
            25,
        ),
        (
            "first_eis",
            "Eis break",
            "Logged the first Eis.",
            "Treat yourself to an Eis.",
            "beverage_count:eis",
            1,
        ),
        (
            "eis_regular",
            "Freezer regular",
            "Logged 10 Eis.",
            "Log 10 Eis.",
            "beverage_count:eis",
            10,
        ),
        (
            "eis_fan",
            "Eis fan",
            "Logged 25 Eis.",
            "Log 25 Eis.",
            "beverage_count:eis",
            25,
        ),
        (
            "first_tea",
            "Tea time",
            "Logged the first Tea.",
            "Try a Tea.",
            "beverage_count:tea",
            1,
        ),
        (
            "tea_regular",
            "Tea regular",
            "Logged 10 Tea.",
            "Log 10 Tea.",
            "beverage_count:tea",
            10,
        ),
        (
            "tea_fan",
            "Tea fan",
            "Logged 50 Tea.",
            "Log 50 Tea.",
            "beverage_count:tea",
            50,
        ),
        (
            "full_menu",
            "Full menu",
            "Logged every regular menu option.",
            "Try Coffee, Espresso, Eis, and Tea.",
            "regular_menu_variety",
            4,
        ),
        (
            "two_week_streak",
            "Two-week streak",
            "Logged drinks on 14 consecutive days.",
            "Keep a 14-day streak.",
            "streak_days",
            14,
        ),
        (
            "month_streak",
            "Monthly ritual",
            "Logged drinks on 30 consecutive days.",
            "Keep a 30-day streak.",
            "streak_days",
            30,
        ),
        (
            "busy_day_10",
            "Busy day",
            "Logged 10 drinks in one day.",
            "Log 10 drinks in one day.",
            "single_day_count",
            10,
        ),
        (
            "double_century",
            "Double century",
            "Logged 200 coffees.",
            "Reach 200 coffees.",
            "lifetime_cups",
            200,
        ),
        (
            "five_hundred",
            "Five hundred",
            "Logged 500 coffees.",
            "Reach 500 coffees.",
            "lifetime_cups",
            500,
        ),
        (
            "fifth_payment",
            "Payment regular",
            "Marked five coffee payments as paid.",
            "Settle five coffee tabs.",
            "payment_count",
            5,
        ),
        (
            "tenth_payment",
            "Clean slate regular",
            "Marked 10 coffee payments as paid.",
            "Settle 10 coffee tabs.",
            "payment_count",
            10,
        ),
        (
            "ten_euro_settlement",
            "Big clean slate",
            "Paid off at least 10 EUR in one payment.",
            "Settle a tab of at least 10 EUR.",
            "single_payment_amount",
            1000,
        ),
    ];

    for (id, name, description, hint, criteria_kind, threshold) in definitions {
        sqlx::query(
            "INSERT OR IGNORE INTO achievement_definitions
             (id, name, description, hint, criteria_kind, threshold, active, created_at)
             VALUES (?, ?, ?, ?, ?, ?, 1, ?)",
        )
        .bind(id)
        .bind(name)
        .bind(description)
        .bind(hint)
        .bind(criteria_kind)
        .bind(threshold)
        .bind(now)
        .execute(pool)
        .await?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[tokio::test]
    async fn seed_defaults_inserts_new_achievement_definitions() {
        let db_path = format!("/tmp/coffee-db-seed-{}.db", Uuid::new_v4());
        let db = connect(&format!("sqlite://{db_path}")).await.unwrap();

        seed_defaults(&db).await.unwrap();

        let ids = [
            "three_day_streak",
            "half_century",
            "triple_day",
            "espresso_pads_fan",
            "eis_fan",
            "first_tea",
            "tea_regular",
            "tea_fan",
            "tenth_payment",
            "morning_regular",
            "lunch_break",
            "weekend_visit",
            "welcome_back",
        ];
        for id in ids {
            let row: Option<(String,)> =
                sqlx::query_as("SELECT id FROM achievement_definitions WHERE id = ?")
                    .bind(id)
                    .fetch_optional(&db)
                    .await
                    .unwrap();
            assert_eq!(row.as_ref().map(|(value,)| value.as_str()), Some(id));
        }
    }
}
