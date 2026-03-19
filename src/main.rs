use axum::{
    extract::{Path, State},
    http::Method,
    routing::{get, post},
    Json, Router,
};
use chrono::{Duration, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use std::net::SocketAddr;
use tower_http::cors::{Any, CorsLayer};

#[derive(Clone)]
struct AppState {
    db: SqlitePool,
}

#[derive(Serialize)]
struct ApiResponse<T> {
    success: bool,
    message: String,
    data: Option<T>,
}

#[derive(Debug, Serialize, Deserialize, FromRow)]
struct Profile {
    id: i64,
    name: String,
    age: i64,
    body_weight: f64,
    xp: i64,
    created_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct CreateProfile {
    name: String,
    age: i64,
    body_weight: f64,
}

#[derive(Debug, Serialize, Deserialize, FromRow)]
struct Exercise {
    id: i64,
    profile_id: i64,
    name: String,
    muscle_group: String,
    created_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct CreateExercise {
    profile_id: i64,
    name: String,
    muscle_group: String,
}

#[derive(Debug, Serialize, Deserialize, FromRow)]
struct WorkoutLog {
    id: i64,
    profile_id: i64,
    exercise_id: i64,
    session_id: i64,
    weight: f64,
    reps: i64,
    total_volume: f64,
    performed_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct CreateWorkoutLog {
    profile_id: i64,
    exercise_id: i64,
    session_id: i64,
    weight: f64,
    reps: i64,
}

#[derive(Debug, Serialize)]
struct ExerciseHistoryItem {
    id: i64,
    date: String,
    weight: f64,
    reps: i64,
    volume: f64,
    is_pr: bool,
}

#[derive(Debug, Serialize)]
struct ExerciseGraphPoint {
    date: String,
    max_weight: f64,
    total_volume: f64,
}

#[derive(Debug, Serialize)]
struct LevelSummary {
    profile_id: i64,
    total_workout_days: i64,
    total_volume: f64,
    current_streak: i64,
    longest_streak: i64,
    pr_count: i64,
    xp: i64,
    score: i64,
    level: String,
}

#[derive(Debug, Serialize, FromRow)]
struct Badge {
    id: i64,
    profile_id: i64,
    name: String,
    description: String,
    unlocked_at: String,
}

#[derive(Debug, Serialize)]
struct DashboardSummary {
    profile_id: i64,
    name: String,
    xp: i64,
    level: String,
    score: i64,
    total_workout_days: i64,
    current_streak: i64,
    longest_streak: i64,
    pr_count: i64,
    total_volume: f64,
    badges: Vec<Badge>,
}

#[derive(Debug, Serialize)]
struct WorkoutLogResponse {
    workout: WorkoutLog,
    gained_xp: i64,
    total_xp: i64,
    new_pr: bool,
    unlocked_badges: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, FromRow, Clone)]
struct ExerciseCatalogItem {
    id: i64,
    name: String,
    muscle_group: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct UpdateExercise {
    name: String,
    muscle_group: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct UpdateWorkoutLog {
    weight: f64,
    reps: i64,
}

#[derive(Debug, Serialize)]
struct MuscleCoverageResponse {
    covered: Vec<String>,
    missing: Vec<String>,
}

#[derive(Debug, Serialize)]
struct Mission {
    name: String,
    description: String,
    completed: bool,
}

#[derive(Debug, Serialize)]
struct MissionSummary {
    profile_id: i64,
    current_streak: i64,
    weekly_missions: Vec<Mission>,
}

#[derive(Debug, Serialize, Deserialize, FromRow, Clone)]
struct WorkoutSession {
    id: i64,
    profile_id: i64,
    started_at: String,
    ended_at: Option<String>,
    status: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct StartWorkoutSessionRequest {
    profile_id: i64,
}

#[derive(Debug, Serialize)]
struct ActiveWorkoutResponse {
    session: WorkoutSession,
    entries: Vec<WorkoutLogWithExercise>,
}

#[derive(Debug, Serialize, FromRow)]
struct WorkoutLogWithExercise {
    id: i64,
    profile_id: i64,
    exercise_id: i64,
    session_id: i64,
    exercise_name: String,
    muscle_group: String,
    weight: f64,
    reps: i64,
    total_volume: f64,
    performed_at: String,
}

#[tokio::main]
async fn main() {
    let db = SqlitePool::connect("sqlite://gym_app.db")
        .await
        .expect("Failed to connect to SQLite");

    init_db(&db).await.expect("Failed to initialize DB");

    let state = AppState { db };

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST])
        .allow_headers(Any);

    let app = Router::new()
        .route("/", get(health_check))
        .route("/profiles", post(create_profile))
        .route("/profiles/{id}", get(get_profile))
        .route("/profiles/{id}/level", get(get_profile_level))
        .route("/profiles/{id}/dashboard", get(get_dashboard))
        .route("/profiles/{id}/badges", get(get_badges))
        .route("/profiles/{id}/exercises", get(get_profile_exercises))
        .route("/profiles/{id}/coverage/today", get(get_today_coverage))
        .route("/profiles/{id}/coverage/week", get(get_week_coverage))
        .route("/profiles/{id}/missions", get(get_missions))
        .route("/catalog/exercises", get(search_catalog_exercises))
        .route("/exercises", post(create_exercise))
        .route("/exercises/{id}", post(update_exercise))
        .route("/exercises/{id}/delete", post(delete_exercise))
        .route("/exercises/{id}/history", get(get_exercise_history))
        .route("/exercises/{id}/graph", get(get_exercise_graph))
        .route("/workouts/start", post(start_workout_session))
        .route("/workouts/active/{profile_id}", get(get_active_workout_session))
        .route("/workouts/end/{session_id}", post(end_workout_session))
        .route("/workouts/log", post(create_workout_log))
        .route("/workouts/{id}", post(update_workout_log))
        .route("/workouts/{id}/delete", post(delete_workout_log))
        .with_state(state)
        .layer(cors);

    let addr: SocketAddr = "127.0.0.1:3000".parse().unwrap();
    println!("Server running at http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn init_db(db: &SqlitePool) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS profiles (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            age INTEGER NOT NULL,
            body_weight REAL NOT NULL,
            xp INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL
        );
        "#,
    )
    .execute(db)
    .await?;

    let _ = sqlx::query(r#"ALTER TABLE profiles ADD COLUMN xp INTEGER NOT NULL DEFAULT 0"#)
        .execute(db)
        .await;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS exercises (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            profile_id INTEGER NOT NULL,
            name TEXT NOT NULL,
            muscle_group TEXT NOT NULL,
            created_at TEXT NOT NULL,
            FOREIGN KEY (profile_id) REFERENCES profiles(id)
        );
        "#,
    )
    .execute(db)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS workout_sessions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            profile_id INTEGER NOT NULL,
            started_at TEXT NOT NULL,
            ended_at TEXT,
            status TEXT NOT NULL,
            FOREIGN KEY (profile_id) REFERENCES profiles(id)
        );
        "#,
    )
    .execute(db)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS workout_logs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            profile_id INTEGER NOT NULL,
            exercise_id INTEGER NOT NULL,
            session_id INTEGER NOT NULL,
            weight REAL NOT NULL,
            reps INTEGER NOT NULL,
            total_volume REAL NOT NULL,
            performed_at TEXT NOT NULL,
            FOREIGN KEY (profile_id) REFERENCES profiles(id),
            FOREIGN KEY (exercise_id) REFERENCES exercises(id),
            FOREIGN KEY (session_id) REFERENCES workout_sessions(id)
        );
        "#,
    )
    .execute(db)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS badges (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            profile_id INTEGER NOT NULL,
            name TEXT NOT NULL,
            description TEXT NOT NULL,
            unlocked_at TEXT NOT NULL,
            UNIQUE(profile_id, name),
            FOREIGN KEY (profile_id) REFERENCES profiles(id)
        );
        "#,
    )
    .execute(db)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS exercise_catalog (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL UNIQUE,
            muscle_group TEXT NOT NULL
        );
        "#,
    )
    .execute(db)
    .await?;

    let default_exercises = vec![
        ("Bench Press", "Chest"),
        ("Incline Bench Press", "Chest"),
        ("Push Up", "Chest"),
        ("Chest Fly", "Chest"),
        ("Squat", "Legs"),
        ("Leg Press", "Legs"),
        ("Lunge", "Legs"),
        ("Romanian Deadlift", "Hamstrings"),
        ("Deadlift", "Back"),
        ("Pull Up", "Back"),
        ("Lat Pulldown", "Back"),
        ("Barbell Row", "Back"),
        ("Shoulder Press", "Shoulders"),
        ("Lateral Raise", "Shoulders"),
        ("Rear Delt Fly", "Shoulders"),
        ("Barbell Curl", "Biceps"),
        ("Hammer Curl", "Biceps"),
        ("Tricep Pushdown", "Triceps"),
        ("Skull Crusher", "Triceps"),
        ("Plank", "Core"),
        ("Crunch", "Core"),
        ("Leg Raise", "Core"),
        ("Calf Raise", "Calves"),
        ("Hip Thrust", "Glutes"),
    ];

    for (name, muscle_group) in default_exercises {
        let _ = sqlx::query(
            r#"
            INSERT OR IGNORE INTO exercise_catalog (name, muscle_group)
            VALUES (?, ?)
            "#,
        )
        .bind(name)
        .bind(muscle_group)
        .execute(db)
        .await;
    }

    Ok(())
}

async fn health_check() -> Json<ApiResponse<String>> {
    Json(ApiResponse {
        success: true,
        message: "Gym RPG backend is running".to_string(),
        data: Some("OK".to_string()),
    })
}

async fn create_profile(
    State(state): State<AppState>,
    Json(payload): Json<CreateProfile>,
) -> Json<ApiResponse<Profile>> {
    let now = Utc::now().to_rfc3339();

    let result = sqlx::query(
        r#"
        INSERT INTO profiles (name, age, body_weight, xp, created_at)
        VALUES (?, ?, ?, ?, ?)
        "#,
    )
    .bind(&payload.name)
    .bind(payload.age)
    .bind(payload.body_weight)
    .bind(0_i64)
    .bind(&now)
    .execute(&state.db)
    .await;

    match result {
        Ok(res) => {
            let id = res.last_insert_rowid();
            let profile = sqlx::query_as::<_, Profile>(
                r#"
                SELECT id, name, age, body_weight, xp, created_at
                FROM profiles
                WHERE id = ?
                "#,
            )
            .bind(id)
            .fetch_one(&state.db)
            .await
            .unwrap();

            Json(ApiResponse {
                success: true,
                message: "Profile created".to_string(),
                data: Some(profile),
            })
        }
        Err(e) => Json(ApiResponse {
            success: false,
            message: format!("Failed to create profile: {}", e),
            data: None,
        }),
    }
}

async fn get_profile(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Json<ApiResponse<Profile>> {
    let result = sqlx::query_as::<_, Profile>(
        r#"
        SELECT id, name, age, body_weight, xp, created_at
        FROM profiles
        WHERE id = ?
        "#,
    )
    .bind(id)
    .fetch_one(&state.db)
    .await;

    match result {
        Ok(profile) => Json(ApiResponse {
            success: true,
            message: "Profile fetched".to_string(),
            data: Some(profile),
        }),
        Err(e) => Json(ApiResponse {
            success: false,
            message: format!("Profile not found: {}", e),
            data: None,
        }),
    }
}

async fn create_exercise(
    State(state): State<AppState>,
    Json(payload): Json<CreateExercise>,
) -> Json<ApiResponse<Exercise>> {
    let now = Utc::now().to_rfc3339();

    let result = sqlx::query(
        r#"
        INSERT INTO exercises (profile_id, name, muscle_group, created_at)
        VALUES (?, ?, ?, ?)
        "#,
    )
    .bind(payload.profile_id)
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

async fn create_workout_log(
    State(state): State<AppState>,
    Json(payload): Json<CreateWorkoutLog>,
) -> Json<ApiResponse<WorkoutLogResponse>> {
    let now = Utc::now().to_rfc3339();

    let active_session = sqlx::query_as::<_, WorkoutSession>(
        r#"
        SELECT id, profile_id, started_at, ended_at, status
        FROM workout_sessions
        WHERE id = ? AND profile_id = ? AND status = 'active'
        "#,
    )
    .bind(payload.session_id)
    .bind(payload.profile_id)
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
    .bind(payload.profile_id)
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
    .bind(payload.profile_id)
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
            .bind(payload.profile_id)
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
    let new_pr = previous_max_weight.map(|w| payload.weight > w).unwrap_or(true);
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
    .bind(payload.profile_id)
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
    .bind(payload.profile_id)
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);

    let unlocked_badges = evaluate_and_unlock_badges(&state.db, payload.profile_id)
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

async fn get_exercise_history(
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

async fn get_exercise_graph(
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

async fn get_profile_level(
    State(state): State<AppState>,
    Path(profile_id): Path<i64>,
) -> Json<ApiResponse<LevelSummary>> {
    match build_level_summary(&state.db, profile_id).await {
        Ok(summary) => Json(ApiResponse {
            success: true,
            message: "Level calculated".to_string(),
            data: Some(summary),
        }),
        Err(e) => Json(ApiResponse {
            success: false,
            message: format!("Failed to calculate level: {}", e),
            data: None,
        }),
    }
}

async fn get_badges(
    State(state): State<AppState>,
    Path(profile_id): Path<i64>,
) -> Json<ApiResponse<Vec<Badge>>> {
    let result = sqlx::query_as::<_, Badge>(
        r#"
        SELECT id, profile_id, name, description, unlocked_at
        FROM badges
        WHERE profile_id = ?
        ORDER BY unlocked_at ASC
        "#,
    )
    .bind(profile_id)
    .fetch_all(&state.db)
    .await;

    match result {
        Ok(badges) => Json(ApiResponse {
            success: true,
            message: "Badges fetched".to_string(),
            data: Some(badges),
        }),
        Err(e) => Json(ApiResponse {
            success: false,
            message: format!("Failed to fetch badges: {}", e),
            data: None,
        }),
    }
}

async fn get_dashboard(
    State(state): State<AppState>,
    Path(profile_id): Path<i64>,
) -> Json<ApiResponse<DashboardSummary>> {
    let profile_result = sqlx::query_as::<_, Profile>(
        r#"
        SELECT id, name, age, body_weight, xp, created_at
        FROM profiles
        WHERE id = ?
        "#,
    )
    .bind(profile_id)
    .fetch_one(&state.db)
    .await;

    match profile_result {
        Ok(profile) => {
            let level_summary = build_level_summary(&state.db, profile_id).await;
            let badges_result = sqlx::query_as::<_, Badge>(
                r#"
                SELECT id, profile_id, name, description, unlocked_at
                FROM badges
                WHERE profile_id = ?
                ORDER BY unlocked_at ASC
                "#,
            )
            .bind(profile_id)
            .fetch_all(&state.db)
            .await;

            match (level_summary, badges_result) {
                (Ok(summary), Ok(badges)) => Json(ApiResponse {
                    success: true,
                    message: "Dashboard fetched".to_string(),
                    data: Some(DashboardSummary {
                        profile_id,
                        name: profile.name,
                        xp: profile.xp,
                        level: summary.level,
                        score: summary.score,
                        total_workout_days: summary.total_workout_days,
                        current_streak: summary.current_streak,
                        longest_streak: summary.longest_streak,
                        pr_count: summary.pr_count,
                        total_volume: summary.total_volume,
                        badges,
                    }),
                }),
                (Err(e), _) => Json(ApiResponse {
                    success: false,
                    message: format!("Failed to build dashboard summary: {}", e),
                    data: None,
                }),
                (_, Err(e)) => Json(ApiResponse {
                    success: false,
                    message: format!("Failed to fetch badges: {}", e),
                    data: None,
                }),
            }
        }
        Err(e) => Json(ApiResponse {
            success: false,
            message: format!("Profile not found: {}", e),
            data: None,
        }),
    }
}

async fn build_level_summary(
    db: &SqlitePool,
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

async fn fetch_distinct_workout_dates(
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

    let dates = rows
        .into_iter()
        .filter_map(|d| NaiveDate::parse_from_str(&d, "%Y-%m-%d").ok())
        .collect();

    Ok(dates)
}

fn calculate_current_streak(dates: &[NaiveDate]) -> i64 {
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

fn calculate_longest_streak(dates: &[NaiveDate]) -> i64 {
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

async fn evaluate_and_unlock_badges(
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

fn level_from_score(score: i64) -> String {
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

async fn get_profile_exercises(
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

async fn search_catalog_exercises(
    State(state): State<AppState>,
) -> Json<ApiResponse<Vec<ExerciseCatalogItem>>> {
    let result = sqlx::query_as::<_, ExerciseCatalogItem>(
        r#"
        SELECT id, name, muscle_group
        FROM exercise_catalog
        ORDER BY name ASC
        "#,
    )
    .fetch_all(&state.db)
    .await;

    match result {
        Ok(items) => Json(ApiResponse {
            success: true,
            message: "Catalog fetched".to_string(),
            data: Some(items),
        }),
        Err(e) => Json(ApiResponse {
            success: false,
            message: format!("Failed to fetch catalog: {}", e),
            data: None,
        }),
    }
}

async fn update_exercise(
    State(state): State<AppState>,
    Path(exercise_id): Path<i64>,
    Json(payload): Json<UpdateExercise>,
) -> Json<ApiResponse<Exercise>> {
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

async fn delete_exercise(
    State(state): State<AppState>,
    Path(exercise_id): Path<i64>,
) -> Json<ApiResponse<String>> {
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

async fn update_workout_log(
    State(state): State<AppState>,
    Path(workout_id): Path<i64>,
    Json(payload): Json<UpdateWorkoutLog>,
) -> Json<ApiResponse<WorkoutLog>> {
    let existing = sqlx::query_as::<_, WorkoutLog>(
        r#"
        SELECT id, profile_id, exercise_id, session_id, weight, reps, total_volume, performed_at
        FROM workout_logs
        WHERE id = ?
        "#,
    )
    .bind(workout_id)
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

async fn delete_workout_log(
    State(state): State<AppState>,
    Path(workout_id): Path<i64>,
) -> Json<ApiResponse<String>> {
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

async fn get_today_coverage(
    State(state): State<AppState>,
    Path(profile_id): Path<i64>,
) -> Json<ApiResponse<MuscleCoverageResponse>> {
    let today = Utc::now().date_naive().format("%Y-%m-%d").to_string();

    let covered: Vec<String> = sqlx::query_scalar(
        r#"
        SELECT DISTINCT e.muscle_group
        FROM workout_logs w
        JOIN exercises e ON w.exercise_id = e.id
        WHERE w.profile_id = ? AND substr(w.performed_at, 1, 10) = ?
        ORDER BY e.muscle_group ASC
        "#,
    )
    .bind(profile_id)
    .bind(today)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let all_groups = standard_muscle_groups();
    let missing = all_groups
        .into_iter()
        .filter(|g| !covered.contains(g))
        .collect::<Vec<_>>();

    Json(ApiResponse {
        success: true,
        message: "Today's coverage fetched".to_string(),
        data: Some(MuscleCoverageResponse { covered, missing }),
    })
}

async fn get_week_coverage(
    State(state): State<AppState>,
    Path(profile_id): Path<i64>,
) -> Json<ApiResponse<MuscleCoverageResponse>> {
    let today = Utc::now().date_naive();
    let start = (today - Duration::days(6)).format("%Y-%m-%d").to_string();
    let end = today.format("%Y-%m-%d").to_string();

    let covered: Vec<String> = sqlx::query_scalar(
        r#"
        SELECT DISTINCT e.muscle_group
        FROM workout_logs w
        JOIN exercises e ON w.exercise_id = e.id
        WHERE w.profile_id = ?
          AND substr(w.performed_at, 1, 10) >= ?
          AND substr(w.performed_at, 1, 10) <= ?
        ORDER BY e.muscle_group ASC
        "#,
    )
    .bind(profile_id)
    .bind(start)
    .bind(end)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let all_groups = standard_muscle_groups();
    let missing = all_groups
        .into_iter()
        .filter(|g| !covered.contains(g))
        .collect::<Vec<_>>();

    Json(ApiResponse {
        success: true,
        message: "Weekly coverage fetched".to_string(),
        data: Some(MuscleCoverageResponse { covered, missing }),
    })
}

async fn get_missions(
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

fn standard_muscle_groups() -> Vec<String> {
    vec![
        "Chest".to_string(),
        "Back".to_string(),
        "Legs".to_string(),
        "Shoulders".to_string(),
        "Biceps".to_string(),
        "Triceps".to_string(),
        "Core".to_string(),
        "Hamstrings".to_string(),
        "Calves".to_string(),
        "Glutes".to_string(),
    ]
}

async fn start_workout_session(
    State(state): State<AppState>,
    Json(payload): Json<StartWorkoutSessionRequest>,
) -> Json<ApiResponse<WorkoutSession>> {
    let existing = sqlx::query_as::<_, WorkoutSession>(
        r#"
        SELECT id, profile_id, started_at, ended_at, status
        FROM workout_sessions
        WHERE profile_id = ? AND status = 'active'
        ORDER BY started_at DESC
        LIMIT 1
        "#,
    )
    .bind(payload.profile_id)
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
    .bind(payload.profile_id)
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

async fn get_active_workout_session(
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

async fn end_workout_session(
    State(state): State<AppState>,
    Path(session_id): Path<i64>,
) -> Json<ApiResponse<WorkoutSession>> {
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