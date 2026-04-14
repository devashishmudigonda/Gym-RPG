

use axum::extract::{State, Extension, Path, Json};
use chrono::Utc;
use crate::{evaluate_and_unlock_badges};
use crate::class::{AppState, ApiResponse, StartWorkoutSessionRequest, WorkoutSession, CreateWorkoutLog, UpdateWorkoutLog, WorkoutLog, WorkoutLogResponse, ActiveWorkoutResponse, WorkoutLogWithExercise};
use crate::exercise::{get_current_profile_by_user_id,get_owned_exercise};


pub async fn get_owned_workout_session(
    db: &sqlx::SqlitePool,
    user_id: i64,
    session_id: i64,
) -> Result<WorkoutSession, sqlx::Error> {
    sqlx::query_as::<_, WorkoutSession>(
        r#"
        SELECT ws.id, ws.profile_id, ws.started_at, ws.ended_at, ws.status
        FROM workout_sessions ws
        JOIN profiles p ON ws.profile_id = p.id
        WHERE ws.id = ? AND p.user_id = ?
        "#,
    )
    .bind(session_id)
    .bind(user_id)
    .fetch_one(db)
    .await
}

pub async fn start_workout_session(
    State(state): State<AppState>,
    Extension(user_id): Extension<i64>,
    Json(_payload): Json<StartWorkoutSessionRequest>,
) -> Json<ApiResponse<WorkoutSession>> {
    let profile = match get_current_profile_by_user_id(&state.db, user_id).await {
        Ok(p) => p,
        Err(e) => {
            return Json(ApiResponse {
                success: false,
                message: format!("Profile fetch failed: {}", e),
                data: None,
            })
        }
    };

    let existing = sqlx::query_as::<_, WorkoutSession>(
        r#"
        SELECT id, profile_id, started_at, ended_at, status
        FROM workout_sessions
        WHERE profile_id = ? AND status = 'active'
        ORDER BY started_at DESC
        LIMIT 1
        "#,
    )
    .bind(profile.id)
    .fetch_optional(&state.db)
    .await;

    match existing {
        Ok(Some(session)) => {
            return Json(ApiResponse {
                success: true,
                message: "Active workout session already exists".to_string(),
                data: Some(session),
            });
        }
        Ok(None) => {}
        Err(e) => {
            return Json(ApiResponse {
                success: false,
                message: format!("Failed to check active session: {}", e),
                data: None,
            });
        }
    }

    let now = Utc::now().to_rfc3339();

    let result = sqlx::query(
        r#"
        INSERT INTO workout_sessions (profile_id, started_at, ended_at, status)
        VALUES (?, ?, NULL, 'active')
        "#,
    )
    .bind(profile.id)
    .bind(&now)
    .execute(&state.db)
    .await;

    match result {
        Ok(res) => {
            let id = res.last_insert_rowid();
            let session = sqlx::query_as::<_, WorkoutSession>(
                r#"
                SELECT id, profile_id, started_at, ended_at, status
                FROM workout_sessions
                WHERE id = ?
                "#,
            )
            .bind(id)
            .fetch_one(&state.db)
            .await
            .unwrap();

            Json(ApiResponse {
                success: true,
                message: "Workout session started".to_string(),
                data: Some(session),
            })
        }
        Err(e) => Json(ApiResponse {
            success: false,
            message: format!("Failed to start workout session: {}", e),
            data: None,
        }),
    }
}

pub async fn end_workout_session(
    State(state): State<AppState>,
    Extension(user_id): Extension<i64>,
    Path(session_id): Path<i64>,
) -> Json<ApiResponse<WorkoutSession>> {
    if get_owned_workout_session(&state.db, user_id, session_id).await.is_err() {
        return Json(ApiResponse {
            success: false,
            message: "Session not found".to_string(),
            data: None,
        });
    }

    let now = Utc::now().to_rfc3339();

    let result = sqlx::query(
        r#"
        UPDATE workout_sessions
        SET ended_at = ?, status = 'completed'
        WHERE id = ? AND status = 'active'
        "#,
    )
    .bind(&now)
    .bind(session_id)
    .execute(&state.db)
    .await;

    match result {
        Ok(res) => {
            if res.rows_affected() == 0 {
                return Json(ApiResponse {
                    success: false,
                    message: "No active session found to end".to_string(),
                    data: None,
                });
            }

            let session = sqlx::query_as::<_, WorkoutSession>(
                r#"
                SELECT id, profile_id, started_at, ended_at, status
                FROM workout_sessions
                WHERE id = ?
                "#,
            )
            .bind(session_id)
            .fetch_one(&state.db)
            .await
            .unwrap();

            Json(ApiResponse {
                success: true,
                message: "Workout session ended".to_string(),
                data: Some(session),
            })
        }
        Err(e) => Json(ApiResponse {
            success: false,
            message: format!("Failed to end workout session: {}", e),
            data: None,
        }),
    }
}


pub async fn create_workout_log(
    State(state): State<AppState>,
    Extension(user_id): Extension<i64>,
    Json(payload): Json<CreateWorkoutLog>,
) -> Json<ApiResponse<WorkoutLogResponse>> {
    let now = Utc::now().to_rfc3339();
    let profile = match get_current_profile_by_user_id(&state.db, user_id).await {
        Ok(p) => p,
        Err(e) => {
            return Json(ApiResponse {
                success: false,
                message: format!("Profile fetch failed: {}", e),
                data: None,
            })
        }
    };

    if get_owned_exercise(&state.db, user_id, payload.exercise_id).await.is_err() {
        return Json(ApiResponse {
            success: false,
            message: "Exercise not found".to_string(),
            data: None,
        });
    }

    if get_owned_workout_session(&state.db, user_id, payload.session_id)
        .await
        .is_err()
    {
        return Json(ApiResponse {
            success: false,
            message: "Workout session not found".to_string(),
            data: None,
        });
    }

    let active_session = sqlx::query_as::<_, WorkoutSession>(
        r#"
        SELECT id, profile_id, started_at, ended_at, status
        FROM workout_sessions
        WHERE id = ? AND profile_id = ? AND status = 'active'
        "#,
    )
    .bind(payload.session_id)
    .bind(profile.id)
    .fetch_optional(&state.db)
    .await;

    match active_session {
        Ok(Some(_)) => {}
        Ok(None) => {
            return Json(ApiResponse {
                success: false,
                message: "No active workout session found. Start a workout first.".to_string(),
                data: None,
            });
        }
        Err(e) => {
            return Json(ApiResponse {
                success: false,
                message: format!("Failed to verify workout session: {}", e),
                data: None,
            });
        }
    }

    let previous_max_weight: Option<f64> = sqlx::query_scalar(
        r#"
        SELECT MAX(weight)
        FROM workout_logs
        WHERE profile_id = ? AND exercise_id = ?
        "#,
    )
    .bind(profile.id)
    .bind(payload.exercise_id)
    .fetch_one(&state.db)
    .await
    .unwrap_or(None);

    let existing_log = sqlx::query_as::<_, WorkoutLog>(
        r#"
        SELECT id, profile_id, exercise_id, session_id, weight, reps, total_volume, performed_at
        FROM workout_logs
        WHERE profile_id = ? AND exercise_id = ? AND session_id = ?
        LIMIT 1
        "#,
    )
    .bind(profile.id)
    .bind(payload.exercise_id)
    .bind(payload.session_id)
    .fetch_optional(&state.db)
    .await;

    let workout = match existing_log {
        Ok(Some(existing)) => {
            let added_volume = payload.weight * payload.reps as f64;
            let updated_weight = existing.weight.max(payload.weight);
            let updated_reps = existing.reps + payload.reps;
            let updated_total_volume = existing.total_volume + added_volume;

            sqlx::query(
                r#"
                UPDATE workout_logs
                SET weight = ?, reps = ?, total_volume = ?, performed_at = ?
                WHERE id = ?
                "#,
            )
            .bind(updated_weight)
            .bind(updated_reps)
            .bind(updated_total_volume)
            .bind(&now)
            .bind(existing.id)
            .execute(&state.db)
            .await
            .unwrap();

            sqlx::query_as::<_, WorkoutLog>(
                r#"
                SELECT id, profile_id, exercise_id, session_id, weight, reps, total_volume, performed_at
                FROM workout_logs
                WHERE id = ?
                "#,
            )
            .bind(existing.id)
            .fetch_one(&state.db)
            .await
            .unwrap()
        }
        Ok(None) => {
            let total_volume = payload.weight * payload.reps as f64;

            let res = sqlx::query(
                r#"
                INSERT INTO workout_logs (profile_id, exercise_id, session_id, weight, reps, total_volume, performed_at)
                VALUES (?, ?, ?, ?, ?, ?, ?)
                "#,
            )
            .bind(profile.id)
            .bind(payload.exercise_id)
            .bind(payload.session_id)
            .bind(payload.weight)
            .bind(payload.reps)
            .bind(total_volume)
            .bind(&now)
            .execute(&state.db)
            .await
            .unwrap();

            let id = res.last_insert_rowid();

            sqlx::query_as::<_, WorkoutLog>(
                r#"
                SELECT id, profile_id, exercise_id, session_id, weight, reps, total_volume, performed_at
                FROM workout_logs
                WHERE id = ?
                "#,
            )
            .bind(id)
            .fetch_one(&state.db)
            .await
            .unwrap()
        }
        Err(e) => {
            return Json(ApiResponse {
                success: false,
                message: format!("Failed to check existing workout log: {}", e),
                data: None,
            });
        }
    };

    let added_volume = payload.weight * payload.reps as f64;
    let volume_xp = (added_volume / 10.0).floor() as i64;
    let rep_xp = payload.reps;
    let base_xp = 10_i64;
    let new_pr = previous_max_weight
        .map(|w| payload.weight > w)
        .unwrap_or(true);
    let pr_xp = if new_pr { 25 } else { 0 };
    let gained_xp = base_xp + volume_xp + rep_xp + pr_xp;

    sqlx::query(
        r#"
        UPDATE profiles
        SET xp = xp + ?
        WHERE id = ?
        "#,
    )
    .bind(gained_xp)
    .bind(profile.id)
    .execute(&state.db)
    .await
    .unwrap();

    let total_xp: i64 = sqlx::query_scalar(
        r#"
        SELECT xp
        FROM profiles
        WHERE id = ?
        "#,
    )
    .bind(profile.id)
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);

    let unlocked_badges = evaluate_and_unlock_badges(&state.db, profile.id)
        .await
        .unwrap_or_default();

    Json(ApiResponse {
        success: true,
        message: "Workout logged".to_string(),
        data: Some(WorkoutLogResponse {
            workout,
            gained_xp,
            total_xp,
            new_pr,
            unlocked_badges,
        }),
    })
}


pub async fn update_workout_log(
    State(state): State<AppState>,
    Extension(user_id): Extension<i64>,
    Path(workout_id): Path<i64>,
    Json(payload): Json<UpdateWorkoutLog>,
) -> Json<ApiResponse<WorkoutLog>> {
    let existing = sqlx::query_as::<_, WorkoutLog>(
        r#"
        SELECT wl.id, wl.profile_id, wl.exercise_id, wl.session_id, wl.weight, wl.reps, wl.total_volume, wl.performed_at
        FROM workout_logs wl
        JOIN profiles p ON wl.profile_id = p.id
        WHERE wl.id = ? AND p.user_id = ?
        "#,
    )
    .bind(workout_id)
    .bind(user_id)
    .fetch_one(&state.db)
    .await;

    match existing {
        Ok(existing_row) => {
            let recalculated_total_volume = payload.weight * payload.reps as f64;

            let result = sqlx::query(
                r#"
                UPDATE workout_logs
                SET weight = ?, reps = ?, total_volume = ?
                WHERE id = ?
                "#,
            )
            .bind(payload.weight)
            .bind(payload.reps)
            .bind(recalculated_total_volume)
            .bind(workout_id)
            .execute(&state.db)
            .await;

            match result {
                Ok(_) => {
                    let workout = sqlx::query_as::<_, WorkoutLog>(
                        r#"
                        SELECT id, profile_id, exercise_id, session_id, weight, reps, total_volume, performed_at
                        FROM workout_logs
                        WHERE id = ?
                        "#,
                    )
                    .bind(existing_row.id)
                    .fetch_one(&state.db)
                    .await
                    .unwrap();

                    Json(ApiResponse {
                        success: true,
                        message: "Workout updated".to_string(),
                        data: Some(workout),
                    })
                }
                Err(e) => Json(ApiResponse {
                    success: false,
                    message: format!("Failed to update workout: {}", e),
                    data: None,
                }),
            }
        }
        Err(e) => Json(ApiResponse {
            success: false,
            message: format!("Workout not found: {}", e),
            data: None,
        }),
    }
}


pub async fn delete_workout_log(
    State(state): State<AppState>,
    Extension(user_id): Extension<i64>,
    Path(workout_id): Path<i64>,
) -> Json<ApiResponse<String>> {
    let exists: Option<i64> = sqlx::query_scalar(
        r#"
        SELECT wl.id
        FROM workout_logs wl
        JOIN profiles p ON wl.profile_id = p.id
        WHERE wl.id = ? AND p.user_id = ?
        "#,
    )
    .bind(workout_id)
    .bind(user_id)
    .fetch_optional(&state.db)
    .await
    .unwrap_or(None);

    if exists.is_none() {
        return Json(ApiResponse {
            success: false,
            message: "Workout not found".to_string(),
            data: None,
        });
    }

    let result = sqlx::query(
        r#"
        DELETE FROM workout_logs
        WHERE id = ?
        "#,
    )
    .bind(workout_id)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => Json(ApiResponse {
            success: true,
            message: "Workout deleted".to_string(),
            data: Some("Deleted".to_string()),
        }),
        Err(e) => Json(ApiResponse {
            success: false,
            message: format!("Failed to delete workout: {}", e),
            data: None,
        }),
    }
}

pub async fn get_active_workout_session(
    State(state): State<AppState>,
    Path(profile_id): Path<i64>,
) -> Json<ApiResponse<ActiveWorkoutResponse>> {
    let session_result = sqlx::query_as::<_, WorkoutSession>(
        r#"
        SELECT id, profile_id, started_at, ended_at, status
        FROM workout_sessions
        WHERE profile_id = ? AND status = 'active'
        ORDER BY started_at DESC
        LIMIT 1
        "#,
    )
    .bind(profile_id)
    .fetch_optional(&state.db)
    .await;

    match session_result {
        Ok(Some(session)) => {
            let entries = sqlx::query_as::<_, WorkoutLogWithExercise>(
                r#"
                SELECT
                    w.id,
                    w.profile_id,
                    w.exercise_id,
                    w.session_id,
                    e.name as exercise_name,
                    e.muscle_group as muscle_group,
                    w.weight,
                    w.reps,
                    w.total_volume,
                    w.performed_at
                FROM workout_logs w
                JOIN exercises e ON w.exercise_id = e.id
                WHERE w.session_id = ?
                ORDER BY w.performed_at ASC
                "#,
            )
            .bind(session.id)
            .fetch_all(&state.db)
            .await
            .unwrap_or_default();

            Json(ApiResponse {
                success: true,
                message: "Active workout fetched".to_string(),
                data: Some(ActiveWorkoutResponse { session, entries }),
            })
        }
        Ok(None) => Json(ApiResponse {
            success: true,
            message: "No active workout session".to_string(),
            data: None,
        }),
        Err(e) => Json(ApiResponse {
            success: false,
            message: format!("Failed to fetch active workout session: {}", e),
            data: None,
        }),
    }
}


pub async fn get_my_active_workout(
    State(state): State<AppState>,
    Extension(user_id): Extension<i64>,
) -> Json<ApiResponse<ActiveWorkoutResponse>> {
    let profile = get_current_profile_by_user_id(&state.db, user_id).await.unwrap();
    get_active_workout_session(State(state), Path(profile.id)).await
}

pub async fn get_my_workout_days(
    State(state): State<AppState>,
    Extension(user_id): Extension<i64>,
) -> Json<ApiResponse<Vec<String>>> {
    let profile = match get_current_profile_by_user_id(&state.db, user_id).await {
        Ok(p) => p,
        Err(e) => {
            return Json(ApiResponse {
                success: false,
                message: format!("Profile fetch failed: {}", e),
                data: None,
            })
        }
    };

    let dates: Vec<String> = sqlx::query_scalar(
        r#"
        SELECT DISTINCT substr(performed_at, 1, 10) as workout_date
        FROM workout_logs
        WHERE profile_id = ?
        ORDER BY workout_date DESC
        "#,
    )
    .bind(profile.id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    Json(ApiResponse {
        success: true,
        message: "Workout days fetched".to_string(),
        data: Some(dates),
    })
}

