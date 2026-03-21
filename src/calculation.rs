use sqlx::SqlitePool;
use chrono::{NaiveDate, Utc, Duration};


pub async fn fetch_distinct_workout_dates(
    db: &SqlitePool,
    profile_id: i64,
) -> Result<Vec<NaiveDate>, sqlx::Error> {
    let rows: Vec<String> = sqlx::query_scalar(
        r#"
        SELECT DISTINCT substr(performed_at, 1, 10)
        FROM workout_logs
        WHERE profile_id = ?
        ORDER BY substr(performed_at, 1, 10) ASC
        "#,
    )
    .bind(profile_id)
    .fetch_all(db)
    .await?;

    Ok(rows
        .into_iter()
        .filter_map(|d| NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok())
        .collect())
}

pub fn calculate_current_streak(dates: &[NaiveDate]) -> i64 {
    if dates.is_empty() {
        return 0;
    }

    let today = Utc::now().date_naive();
    let yesterday = today - Duration::days(1);

    let mut streak = 0_i64;
    let mut expected = if dates.contains(&today) {
        today
    } else if dates.contains(&yesterday) {
        yesterday
    } else {
        return 0;
    };

    for d in dates.iter().rev() {
        if *d == expected {
            streak += 1;
            expected -= Duration::days(1);
        } else if *d < expected {
            break;
        }
    }

    streak
}

pub fn calculate_longest_streak(dates: &[NaiveDate]) -> i64 {
    if dates.is_empty() {
        return 0;
    }

    let mut longest = 1_i64;
    let mut current = 1_i64;

    for i in 1..dates.len() {
        if dates[i] == dates[i - 1] + Duration::days(1) {
            current += 1;
            if current > longest {
                longest = current;
            }
        } else {
            current = 1;
        }
    }

    longest
}

pub fn level_from_score(score: i64) -> String {
    match score {
        0..=49 => "Beginner".to_string(),
        50..=119 => "Amateur".to_string(),
        120..=249 => "Novice".to_string(),
        250..=449 => "Intermediate".to_string(),
        450..=699 => "Advanced".to_string(),
        700..=999 => "Elite".to_string(),
        1000..=1499 => "Titan".to_string(),
        _ => "Olympian".to_string(),
    }
}
