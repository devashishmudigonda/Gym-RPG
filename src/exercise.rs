use axum::extract::{State, Extension, Path, Json};
use chrono::Utc;
use crate::class::{AppState, ApiResponse, Exercise, CreateExercise, UpdateExercise, Profile, ExerciseGraphPoint, WorkoutLog, ExerciseHistoryItem};

pub async fn get_current_profile_by_user_id(db: &sqlx::SqlitePool, user_id: i64) -> Result<Profile, sqlx::Error> {
    sqlx::query_as::<_, Profile>(
        "SELECT id, name, age, body_weight, xp, created_at FROM profiles WHERE user_id = ?"
    )
    .bind(user_id)
    .fetch_one(db)
    .await
}

pub async fn get_owned_exercise(db: &sqlx::SqlitePool, user_id: i64, exercise_id: i64) -> Result<Exercise, sqlx::Error> {
    sqlx::query_as::<_, Exercise>(
        "SELECT e.id, e.profile_id, e.name, e.muscle_group, e.created_at FROM exercises e JOIN profiles p ON e.profile_id = p.id WHERE e.id = ? AND p.user_id = ?"
    )
    .bind(exercise_id)
    .bind(user_id)
    .fetch_one(db)
    .await
}

pub async fn get_profile_exercises(
    State(state): State<AppState>,
    Path(profile_id): Path<i64>,
) -> Json<ApiResponse<Vec<Exercise>>> {
    let result = sqlx::query_as::<_, Exercise>(
        r#"
        SELECT id, profile_id, name, muscle_group, created_at
        FROM exercises
        WHERE profile_id = ?
        ORDER BY name ASC
        "#,
    )
    .bind(profile_id)
    .fetch_all(&state.db)
    .await;

    match result {
        Ok(exercises) => Json(ApiResponse {
            success: true,
            message: "Exercises fetched".to_string(),
            data: Some(exercises),
        }),
        Err(e) => Json(ApiResponse {
            success: false,
            message: format!("Failed to fetch exercises: {}", e),
            data: None,
        }),
    }
}

pub async fn get_exercise_history(
    State(state): State<AppState>,
    Path(exercise_id): Path<i64>,
) -> Json<ApiResponse<Vec<ExerciseHistoryItem>>> {
    let rows = sqlx::query_as::<_, WorkoutLog>(
        r#"
        SELECT id, profile_id, exercise_id, session_id, weight, reps, total_volume, performed_at
        FROM workout_logs
        WHERE exercise_id = ?
        ORDER BY performed_at ASC
        "#,
    )
    .bind(exercise_id)
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(logs) => {
            let mut running_max = 0.0_f64;
            let mut history = Vec::new();

            for log in logs {
                let is_pr = log.weight > running_max;
                if log.weight > running_max {
                    running_max = log.weight;
                }

                history.push(ExerciseHistoryItem {
                    id: log.id,
                    date: log.performed_at,
                    weight: log.weight,
                    reps: log.reps,
                    volume: log.total_volume,
                    is_pr,
                });
            }

            Json(ApiResponse {
                success: true,
                message: "Exercise history fetched".to_string(),
                data: Some(history),
            })
        }
        Err(e) => Json(ApiResponse {
            success: false,
            message: format!("Failed to fetch history: {}", e),
            data: None,
        }),
    }
}

pub async fn get_exercise_graph(
    State(state): State<AppState>,
    Path(exercise_id): Path<i64>,
) -> Json<ApiResponse<Vec<ExerciseGraphPoint>>> {
    let result = sqlx::query_as::<_, (String, f64, f64)>(
        r#"
        SELECT
            substr(performed_at, 1, 10) as day,
            MAX(weight) as max_weight,
            SUM(total_volume) as total_volume
        FROM workout_logs
        WHERE exercise_id = ?
        GROUP BY substr(performed_at, 1, 10)
        ORDER BY day ASC
        "#,
    )
    .bind(exercise_id)
    .fetch_all(&state.db)
    .await;

    match result {
        Ok(rows) => {
            let points = rows
                .into_iter()
                .map(|(date, max_weight, total_volume)| ExerciseGraphPoint {
                    date,
                    max_weight,
                    total_volume,
                })
                .collect();

            Json(ApiResponse {
                success: true,
                message: "Exercise graph data fetched".to_string(),
                data: Some(points),
            })
        }
        Err(e) => Json(ApiResponse {
            success: false,
            message: format!("Failed to fetch graph data: {}", e),
            data: None,
        }),
    }
}

pub async fn create_exercise(
    State(state): State<AppState>,
    Extension(user_id): Extension<i64>,
    Json(payload): Json<CreateExercise>,
) -> Json<ApiResponse<Exercise>> {
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

    let result = sqlx::query(
        r#"
        INSERT INTO exercises (profile_id, name, muscle_group, created_at)
        VALUES (?, ?, ?, ?)
        "#,
    )
    .bind(profile.id)
    .bind(&payload.name)
    .bind(&payload.muscle_group)
    .bind(&now)
    .execute(&state.db)
    .await;

    match result {
        Ok(res) => {
            let id = res.last_insert_rowid();
            let exercise = sqlx::query_as::<_, Exercise>(
                r#"
                SELECT id, profile_id, name, muscle_group, created_at
                FROM exercises
                WHERE id = ?
                "#,
            )
            .bind(id)
            .fetch_one(&state.db)
            .await
            .unwrap();

            Json(ApiResponse {
                success: true,
                message: "Exercise created".to_string(),
                data: Some(exercise),
            })
        }
        Err(e) => Json(ApiResponse {
            success: false,
            message: format!("Failed to create exercise: {}", e),
            data: None,
        }),
    }
}

pub async fn update_exercise(
    State(state): State<AppState>,
    Extension(user_id): Extension<i64>,
    Path(exercise_id): Path<i64>,
    Json(payload): Json<UpdateExercise>,
) -> Json<ApiResponse<Exercise>> {
    if get_owned_exercise(&state.db, user_id, exercise_id).await.is_err() {
        return Json(ApiResponse {
            success: false,
            message: "Exercise not found".to_string(),
            data: None,
        });
    }

    let result = sqlx::query(
        r#"
        UPDATE exercises
        SET name = ?, muscle_group = ?
        WHERE id = ?
        "#,
    )
    .bind(&payload.name)
    .bind(&payload.muscle_group)
    .bind(exercise_id)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => {
            let exercise = sqlx::query_as::<_, Exercise>(
                r#"
                SELECT id, profile_id, name, muscle_group, created_at
                FROM exercises
                WHERE id = ?
                "#,
            )
            .bind(exercise_id)
            .fetch_one(&state.db)
            .await
            .unwrap();

            Json(ApiResponse {
                success: true,
                message: "Exercise updated".to_string(),
                data: Some(exercise),
            })
        }
        Err(e) => Json(ApiResponse {
            success: false,
            message: format!("Failed to update exercise: {}", e),
            data: None,
        }),
    }
}

pub async fn delete_exercise(
    State(state): State<AppState>,
    Extension(user_id): Extension<i64>,
    Path(exercise_id): Path<i64>,
) -> Json<ApiResponse<String>> {
    if get_owned_exercise(&state.db, user_id, exercise_id).await.is_err() {
        return Json(ApiResponse {
            success: false,
            message: "Exercise not found".to_string(),
            data: None,
        });
    }

    let _ = sqlx::query(
        r#"
        DELETE FROM workout_logs
        WHERE exercise_id = ?
        "#,
    )
    .bind(exercise_id)
    .execute(&state.db)
    .await;

    let result = sqlx::query(
        r#"
        DELETE FROM exercises
        WHERE id = ?
        "#,
    )
    .bind(exercise_id)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => Json(ApiResponse {
            success: true,
            message: "Exercise deleted".to_string(),
            data: Some("Deleted".to_string()),
        }),
        Err(e) => Json(ApiResponse {
            success: false,
            message: format!("Failed to delete exercise: {}", e),
            data: None,
        }),
    }
}

