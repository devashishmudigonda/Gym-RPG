use axum::extract::{Extension, Json, Path, State};
use chrono::NaiveDate;
use crate::class::{ApiResponse, AppState, CalendarDay, MonthSummary, UpsertNoteRequest, WorkoutNote};
use crate::exercise::get_current_profile_by_user_id;

#[derive(sqlx::FromRow)]
struct DayRow {
    date: String,
    total_volume: f64,
    session_count: i64,
    muscle_groups: String,
    total_xp_earned: i64,
}

pub async fn get_me_calendar_month(
    State(state): State<AppState>,
    Extension(user_id): Extension<i64>,
    Path((year, month)): Path<(i32, i32)>,
) -> Json<ApiResponse<MonthSummary>> {
    let profile = match get_current_profile_by_user_id(&state.db, user_id).await {
        Ok(p) => p,
        Err(e) => {
            return Json(ApiResponse {
                success: false,
                message: format!("Profile fetch failed: {}", e),
                data: None,
            });
        }
    };

    let start_date = match NaiveDate::from_ymd_opt(year, month as u32, 1) {
        Some(d) => d,
        None => {
            return Json(ApiResponse {
                success: false,
                message: "Invalid year/month".to_string(),
                data: None,
            });
        }
    };

    let end_date = if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1).unwrap()
    } else {
        NaiveDate::from_ymd_opt(year, (month + 1) as u32, 1).unwrap()
    };

    let start_str = start_date.format("%Y-%m-%d").to_string();
    let end_str = end_date.format("%Y-%m-%d").to_string();

    let rows: Vec<DayRow> = sqlx::query_as(
        r#"
        SELECT
            substr(w.performed_at, 1, 10) as date,
            COALESCE(SUM(w.total_volume), 0.0) as total_volume,
            COUNT(DISTINCT w.session_id) as session_count,
            COALESCE(GROUP_CONCAT(DISTINCT e.muscle_group), '') as muscle_groups,
            COALESCE(SUM(CAST(w.reps * w.weight / 10 + 10 AS INTEGER)), 0) as total_xp_earned
        FROM workout_logs w
        JOIN exercises e ON w.exercise_id = e.id
        WHERE w.profile_id = ?
          AND substr(w.performed_at, 1, 10) >= ?
          AND substr(w.performed_at, 1, 10) < ?
        GROUP BY substr(w.performed_at, 1, 10)
        ORDER BY date ASC
        "#,
    )
    .bind(profile.id)
    .bind(&start_str)
    .bind(&end_str)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let mut day_map: std::collections::HashMap<String, &DayRow> = std::collections::HashMap::new();
    for row in &rows {
        day_map.insert(row.date.clone(), row);
    }

    let num_days = (end_date - start_date).num_days();
    let mut days = Vec::new();
    let mut total_workout_days: i64 = 0;
    let mut total_volume: f64 = 0.0;
    let mut best_day_volume: f64 = 0.0;
    let mut current_streak: i64 = 0;
    let mut best_streak: i64 = 0;
    let mut streak: i64 = 0;

    for i in 0..num_days {
        let d = start_date + chrono::Duration::days(i);
        let date_str = d.format("%Y-%m-%d").to_string();

        if let Some(row) = day_map.get(&date_str) {
            let muscle_groups: Vec<String> = row
                .muscle_groups
                .split(',')
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
                .collect();

            days.push(CalendarDay {
                date: date_str,
                total_volume: row.total_volume,
                session_count: row.session_count,
                muscle_groups,
                had_workout: true,
                total_xp_earned: row.total_xp_earned,
            });

            total_workout_days += 1;
            total_volume += row.total_volume;
            if row.total_volume > best_day_volume {
                best_day_volume = row.total_volume;
            }
            streak += 1;
            if streak > best_streak {
                best_streak = streak;
            }
        } else {
            days.push(CalendarDay {
                date: date_str,
                total_volume: 0.0,
                session_count: 0,
                muscle_groups: vec![],
                had_workout: false,
                total_xp_earned: 0,
            });
            streak = 0;
        }
    }

    // current_streak = count consecutive workout days ending at the last day of the month
    current_streak = 0;
    for day in days.iter().rev() {
        if day.had_workout {
            current_streak += 1;
        } else {
            break;
        }
    }

    Json(ApiResponse {
        success: true,
        message: "Calendar month fetched".to_string(),
        data: Some(MonthSummary {
            year,
            month,
            days,
            total_workout_days,
            total_volume,
            best_day_volume,
            best_streak,
            current_streak,
        }),
    })
}

pub async fn get_me_workout_note(
    State(state): State<AppState>,
    Extension(user_id): Extension<i64>,
    Path(date): Path<String>,
) -> Json<ApiResponse<Option<WorkoutNote>>> {
    let profile = match get_current_profile_by_user_id(&state.db, user_id).await {
        Ok(p) => p,
        Err(e) => {
            return Json(ApiResponse {
                success: false,
                message: format!("Profile fetch failed: {}", e),
                data: None,
            });
        }
    };

    let note = sqlx::query_as::<_, WorkoutNote>(
        "SELECT id, profile_id, date, note, created_at FROM workout_notes WHERE profile_id = ? AND date = ?",
    )
    .bind(profile.id)
    .bind(&date)
    .fetch_optional(&state.db)
    .await
    .unwrap_or(None);

    Json(ApiResponse {
        success: true,
        message: "Note fetched".to_string(),
        data: Some(note),
    })
}

pub async fn upsert_workout_note(
    State(state): State<AppState>,
    Extension(user_id): Extension<i64>,
    Json(body): Json<UpsertNoteRequest>,
) -> Json<ApiResponse<WorkoutNote>> {
    let profile = match get_current_profile_by_user_id(&state.db, user_id).await {
        Ok(p) => p,
        Err(e) => {
            return Json(ApiResponse {
                success: false,
                message: format!("Profile fetch failed: {}", e),
                data: None,
            });
        }
    };

    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string();

    let result = sqlx::query(
        r#"
        INSERT INTO workout_notes (profile_id, date, note, created_at)
        VALUES (?, ?, ?, ?)
        ON CONFLICT(profile_id, date) DO UPDATE SET note = excluded.note, created_at = excluded.created_at
        "#,
    )
    .bind(profile.id)
    .bind(&body.date)
    .bind(&body.note)
    .bind(&now)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => {
            let saved = sqlx::query_as::<_, WorkoutNote>(
                "SELECT id, profile_id, date, note, created_at FROM workout_notes WHERE profile_id = ? AND date = ?",
            )
            .bind(profile.id)
            .bind(&body.date)
            .fetch_one(&state.db)
            .await
            .unwrap();

            Json(ApiResponse {
                success: true,
                message: "Note saved".to_string(),
                data: Some(saved),
            })
        }
        Err(e) => Json(ApiResponse {
            success: false,
            message: format!("Failed to save note: {}", e),
            data: None,
        }),
    }
}

pub async fn get_me_year_heatmap(
    State(state): State<AppState>,
    Extension(user_id): Extension<i64>,
    Path(year): Path<i32>,
) -> Json<ApiResponse<Vec<CalendarDay>>> {
    let profile = match get_current_profile_by_user_id(&state.db, user_id).await {
        Ok(p) => p,
        Err(e) => {
            return Json(ApiResponse {
                success: false,
                message: format!("Profile fetch failed: {}", e),
                data: None,
            });
        }
    };

    let start_str = format!("{}-01-01", year);
    let end_str = format!("{}-01-01", year + 1);

    let rows: Vec<DayRow> = sqlx::query_as(
        r#"
        SELECT
            substr(w.performed_at, 1, 10) as date,
            COALESCE(SUM(w.total_volume), 0.0) as total_volume,
            COUNT(DISTINCT w.session_id) as session_count,
            COALESCE(GROUP_CONCAT(DISTINCT e.muscle_group), '') as muscle_groups,
            COALESCE(SUM(CAST(w.reps * w.weight / 10 + 10 AS INTEGER)), 0) as total_xp_earned
        FROM workout_logs w
        JOIN exercises e ON w.exercise_id = e.id
        WHERE w.profile_id = ?
          AND substr(w.performed_at, 1, 10) >= ?
          AND substr(w.performed_at, 1, 10) < ?
        GROUP BY substr(w.performed_at, 1, 10)
        ORDER BY date ASC
        "#,
    )
    .bind(profile.id)
    .bind(&start_str)
    .bind(&end_str)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let mut day_map: std::collections::HashMap<String, &DayRow> = std::collections::HashMap::new();
    for row in &rows {
        day_map.insert(row.date.clone(), row);
    }

    let year_start = NaiveDate::from_ymd_opt(year, 1, 1).unwrap();
    let year_end = NaiveDate::from_ymd_opt(year + 1, 1, 1).unwrap();
    let num_days = (year_end - year_start).num_days();

    let mut days = Vec::new();
    for i in 0..num_days {
        let d = year_start + chrono::Duration::days(i);
        let date_str = d.format("%Y-%m-%d").to_string();

        if let Some(row) = day_map.get(&date_str) {
            let muscle_groups: Vec<String> = row
                .muscle_groups
                .split(',')
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
                .collect();

            days.push(CalendarDay {
                date: date_str,
                total_volume: row.total_volume,
                session_count: row.session_count,
                muscle_groups,
                had_workout: true,
                total_xp_earned: row.total_xp_earned,
            });
        } else {
            days.push(CalendarDay {
                date: date_str,
                total_volume: 0.0,
                session_count: 0,
                muscle_groups: vec![],
                had_workout: false,
                total_xp_earned: 0,
            });
        }
    }

    Json(ApiResponse {
        success: true,
        message: "Year heatmap fetched".to_string(),
        data: Some(days),
    })
}
