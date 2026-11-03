//! Crates earned from drinking activity: one per active day, a bonus for
//! every STREAK_BONUS_EVERY lab days in a row, an Advent calendar door for
//! each active day from 1 to 24 December, and one for every
//! MILESTONE_EVERY-th drink. Pure functions so the rules are easy to test.

use std::collections::BTreeSet;

use chrono::{Datelike, Duration, NaiveDate, TimeZone, Utc, Weekday};
use chrono_tz::Tz;

/// A streak bonus crate at 5, 10, 15, … consecutive lab days.
pub const STREAK_BONUS_EVERY: i64 = 5;
/// A milestone crate for the 25th, 50th, 75th, … drink.
pub const MILESTONE_EVERY: usize = 25;

#[derive(Debug, Default, PartialEq, Eq)]
pub struct ActivityRewards {
    /// Local days with at least one drink logged at or after the cutoff.
    pub daily: Vec<NaiveDate>,
    /// Days on which the lab-day streak reached a multiple of STREAK_BONUS_EVERY.
    pub streak: Vec<NaiveDate>,
    /// Active days between 1 and 24 December.
    pub advent: Vec<NaiveDate>,
    /// (drink number, timestamp) for every milestone drink at or after the cutoff.
    pub milestones: Vec<(usize, i64)>,
}

fn local_date(timestamp: i64, tz: &Tz) -> Option<NaiveDate> {
    Utc.timestamp_opt(timestamp, 0)
        .single()
        .map(|dt| dt.with_timezone(tz).date_naive())
}

fn is_lab_day(date: NaiveDate) -> bool {
    !matches!(date.weekday(), Weekday::Sat | Weekday::Sun)
}

fn next_lab_day(mut date: NaiveDate) -> NaiveDate {
    loop {
        date += Duration::days(1);
        if is_lab_day(date) {
            return date;
        }
    }
}

/// `consumed_at` must be sorted ascending. Only rewards earned at or after
/// `cutoff` are returned, but streaks count the full history so a streak that
/// was already running at the cutoff keeps its length.
pub fn activity_rewards(consumed_at: &[i64], tz: &Tz, cutoff: i64) -> ActivityRewards {
    let mut rewards = ActivityRewards::default();

    let mut all_days = BTreeSet::new();
    let mut rewarded_days = BTreeSet::new();
    for &timestamp in consumed_at {
        let Some(day) = local_date(timestamp, tz) else {
            continue;
        };
        all_days.insert(day);
        if timestamp >= cutoff {
            rewarded_days.insert(day);
        }
    }

    rewards.daily = rewarded_days.iter().copied().collect();
    rewards.advent = rewarded_days
        .iter()
        .copied()
        .filter(|day| day.month() == 12 && day.day() <= 24)
        .collect();

    // Weekends neither extend nor break a streak (same rule as the stats page).
    let mut run = 0i64;
    let mut previous: Option<NaiveDate> = None;
    for day in all_days.iter().copied().filter(|day| is_lab_day(*day)) {
        run = match previous {
            Some(prev) if next_lab_day(prev) == day => run + 1,
            _ => 1,
        };
        previous = Some(day);
        if run % STREAK_BONUS_EVERY == 0 && rewarded_days.contains(&day) {
            rewards.streak.push(day);
        }
    }

    rewards.milestones = consumed_at
        .iter()
        .enumerate()
        .filter(|(index, timestamp)| (index + 1) % MILESTONE_EVERY == 0 && **timestamp >= cutoff)
        .map(|(index, timestamp)| (index + 1, *timestamp))
        .collect();

    rewards
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(tz: &Tz, y: i32, m: u32, d: u32, h: u32) -> i64 {
        tz.with_ymd_and_hms(y, m, d, h, 0, 0).unwrap().timestamp()
    }

    #[test]
    fn one_daily_crate_per_active_day_after_the_cutoff() {
        let tz = chrono_tz::Europe::Berlin;
        let drinks = [
            at(&tz, 2026, 11, 2, 9),
            at(&tz, 2026, 11, 3, 9),
            at(&tz, 2026, 11, 3, 14), // second drink, same day
            at(&tz, 2026, 11, 4, 23), // late evening still counts for the 4th
        ];
        let rewards = activity_rewards(&drinks, &tz, at(&tz, 2026, 11, 3, 0));
        let days: Vec<_> = rewards.daily.iter().map(|d| d.to_string()).collect();
        assert_eq!(days, ["2026-11-03", "2026-11-04"]);
    }

    #[test]
    fn streak_bonus_every_five_lab_days_and_weekends_do_not_break_it() {
        let tz = chrono_tz::Europe::Berlin;
        // Wed 4 Nov .. Fri 13 Nov 2026 on all lab days, nothing on the weekend.
        let mut drinks = Vec::new();
        for day in [4, 5, 6, 9, 10, 11, 12, 13] {
            drinks.push(at(&tz, 2026, 11, day, 9));
        }
        // A gap (Mon 16 Nov missed), then a new run.
        drinks.push(at(&tz, 2026, 11, 17, 9));
        let rewards = activity_rewards(&drinks, &tz, 0);
        let streak: Vec<_> = rewards.streak.iter().map(|d| d.to_string()).collect();
        // Day 5 of the run is Tue 10 Nov; day 10 would be 17 Nov, but the run broke.
        assert_eq!(streak, ["2026-11-10"]);
    }

    #[test]
    fn streaks_running_at_the_cutoff_keep_their_length() {
        let tz = chrono_tz::Europe::Berlin;
        let drinks: Vec<_> = [2, 3, 4, 5, 6]
            .iter()
            .map(|d| at(&tz, 2026, 11, *d, 9))
            .collect();
        // Cutoff on Friday: four days of history count for the run, the bonus is new.
        let rewards = activity_rewards(&drinks, &tz, at(&tz, 2026, 11, 6, 0));
        assert_eq!(rewards.streak.len(), 1);
        assert_eq!(rewards.daily.len(), 1);
    }

    #[test]
    fn advent_doors_open_from_the_1st_to_the_24th() {
        let tz = chrono_tz::Europe::Berlin;
        let drinks = [
            at(&tz, 2026, 11, 30, 9),
            at(&tz, 2026, 12, 1, 9),
            at(&tz, 2026, 12, 24, 9),
            at(&tz, 2026, 12, 25, 9),
        ];
        let rewards = activity_rewards(&drinks, &tz, 0);
        let advent: Vec<_> = rewards.advent.iter().map(|d| d.to_string()).collect();
        assert_eq!(advent, ["2026-12-01", "2026-12-24"]);
    }

    #[test]
    fn milestones_every_25_drinks_after_the_cutoff() {
        let tz = chrono_tz::Europe::Berlin;
        let drinks: Vec<i64> = (0..80).map(|i| 1_000 + i * 60).collect();
        let cutoff = drinks[30]; // drink 31
        let rewards = activity_rewards(&drinks, &tz, cutoff);
        let counts: Vec<_> = rewards.milestones.iter().map(|(n, _)| *n).collect();
        assert_eq!(counts, [50, 75]);
    }
}
