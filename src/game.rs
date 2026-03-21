use axum::extract::{State, Path, Json};
use crate::class::{AppState, ApiResponse, MissionSummary,LevelSummary, Mission};
use crate::{fetch_distinct_workout_dates, calculate_current_streak, calculate_longest_streak, level_from_score};
use chrono::Utc;
use sqlx::SqlitePool;



pub async fn get_missions(
    State(state): State<AppState>,
    Path(profile_id): Path<i64>,
) -> Json<ApiResponse<MissionSummary>> {
    let summary = build_level_summary(&state.db, profile_id).await.unwrap();
    let today = Utc::now().date_naive().format("%Y-%m-%d").to_string();

    let today_logs: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM workout_logs
        WHERE profile_id = ? AND substr(performed_at, 1, 10) = ?
        "#,
    )
    .bind(profile_id)
    .bind(today)
    .fetch_one(&state.db)
    .await
    .unwrap_or(Some(0))
    .unwrap_or(0);

    let week_covered: Vec<String> = sqlx::query_scalar(
        r#"
        SELECT DISTINCT e.muscle_group
        FROM workout_logs w
        JOIN exercises e ON w.exercise_id = e.id
        WHERE w.profile_id = ?
          AND substr(w.performed_at, 1, 10) >= date('now', '-6 day')
        "#,
    )
    .bind(profile_id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let missions = vec![
        Mission {
            name: "Daily Grind".to_string(),
            description: "Log at least 1 workout today".to_string(),
            completed: today_logs >= 1,
        },
        Mission {
            name: "Triple Threat".to_string(),
            description: "Reach a 3-day streak".to_string(),
            completed: summary.current_streak >= 3,
        },
        Mission {
            name: "Balanced Week".to_string(),
            description: "Train at least 4 muscle groups this week".to_string(),
            completed: week_covered.len() >= 4,
        },
    ];

    Json(ApiResponse {
        success: true,
        message: "Missions fetched".to_string(),
        data: Some(MissionSummary {
            profile_id,
            current_streak: summary.current_streak,
            weekly_missions: missions,
        }),
    })
}


pub async fn evaluate_and_unlock_badges(
    db: &SqlitePool,
    profile_id: i64,
) -> Result<Vec<String>, sqlx::Error> {
    let mut unlocked = Vec::new();
    let summary = build_level_summary(db, profile_id).await?;

    let total_logs: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM workout_logs
        WHERE profile_id = ?
        "#,
    )
    .bind(profile_id)
    .fetch_one(db)
    .await
    .unwrap_or(Some(0))
    .unwrap_or(0);

    let max_single_weight: f64 = sqlx::query_scalar(
        r#"
        SELECT MAX(weight)
        FROM workout_logs
        WHERE profile_id = ?
        "#,
    )
    .bind(profile_id)
    .fetch_one(db)
    .await
    .unwrap_or(Some(0.0))
    .unwrap_or(0.0);

    let candidates = vec![
        (
            total_logs >= 1,
            "First Lift",
            "Logged your very first workout",
        ),
        (
            summary.current_streak >= 3,
            "3-Day Streak",
            "Worked out for 3 consecutive days",
        ),
        (
            summary.current_streak >= 7,
            "7-Day Streak",
            "Worked out for 7 consecutive days",
        ),
        (
            summary.pr_count >= 3,
            "PR Hunter",
            "Hit personal records across exercises",
        ),
        (
            max_single_weight >= 100.0,
            "100 KG Club",
            "Lifted 100 kg or more in a logged set",
        ),
        (
            summary.xp >= 500,
            "Rising Warrior",
            "Reached 500 XP",
        ),
        (
            summary.xp >= 1500,
            "Iron Veteran",
            "Reached 1500 XP",
        ),
    ];

    for (condition, name, description) in candidates {
        if condition {
            let now = Utc::now().to_rfc3339();
            let res = sqlx::query(
                r#"
                INSERT OR IGNORE INTO badges (profile_id, name, description, unlocked_at)
                VALUES (?, ?, ?, ?)
                "#,
            )
            .bind(profile_id)
            .bind(name)
            .bind(description)
            .bind(&now)
            .execute(db)
            .await?;

            if res.rows_affected() > 0 {
                unlocked.push(name.to_string());
            }
        }
    }

    Ok(unlocked)
}


pub async fn build_level_summary(
    db: &sqlx::SqlitePool,
    profile_id: i64,
) -> Result<LevelSummary, sqlx::Error> {
    let workout_days: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(DISTINCT substr(performed_at, 1, 10))
        FROM workout_logs
        WHERE profile_id = ?
        "#,
    )
    .bind(profile_id)
    .fetch_one(db)
    .await
    .unwrap_or(Some(0))
    .unwrap_or(0);

    let total_volume: f64 = sqlx::query_scalar(
        r#"
        SELECT SUM(total_volume)
        FROM workout_logs
        WHERE profile_id = ?
        "#,
    )
    .bind(profile_id)
    .fetch_one(db)
    .await
    .unwrap_or(Some(0.0))
    .unwrap_or(0.0);

    let pr_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM (
            SELECT exercise_id, MAX(weight)
            FROM workout_logs
            WHERE profile_id = ?
            GROUP BY exercise_id
        )
        "#,
    )
    .bind(profile_id)
    .fetch_one(db)
    .await
    .unwrap_or(Some(0))
    .unwrap_or(0);

    let xp: i64 = sqlx::query_scalar(
        r#"
        SELECT xp
        FROM profiles
        WHERE id = ?
        "#,
    )
    .bind(profile_id)
    .fetch_one(db)
    .await
    .unwrap_or(0);

    let workout_dates = fetch_distinct_workout_dates(db, profile_id).await?;
    let current_streak = calculate_current_streak(&workout_dates);
    let longest_streak = calculate_longest_streak(&workout_dates);

    let consistency_points = workout_days * 3;
    let streak_points = current_streak * 5;
    let volume_points = (total_volume / 100.0).floor() as i64;
    let pr_points = pr_count * 20;
    let xp_points = xp / 10;

    let score = consistency_points + streak_points + volume_points + pr_points + xp_points;
    let level = level_from_score(score);

    Ok(LevelSummary {
        profile_id,
        total_workout_days: workout_days,
        total_volume,
        current_streak,
        longest_streak,
        pr_count,
        xp,
        score,
        level,
    })
}