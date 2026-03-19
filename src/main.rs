use axum::{
    extract::{Path, State},
    http::Method,
    routing::{get, post},
    Json, Router,
};
use chrono::Utc;
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
    weight: f64,
    reps: i64,
    performed_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct CreateWorkoutLog {
    profile_id: i64,
    exercise_id: i64,
    weight: f64,
    reps: i64,
}

#[derive(Debug, Serialize)]
struct ExerciseHistoryItem {
    date: String,
    weight: f64,
    reps: i64,
    volume: f64,
}

#[derive(Debug, Serialize)]
struct LevelSummary {
    profile_id: i64,
    total_workout_days: i64,
    total_volume: f64,
    pr_count: i64,
    score: i64,
    level: String,
}

#[derive(Debug, Serialize)]
struct ExerciseGraphPoint {
    date: String,
    max_weight: f64,
    total_volume: f64,
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
    .route("/exercises", post(create_exercise))
    .route("/exercises/{id}/history", get(get_exercise_history))
    .route("/exercises/{id}/graph", get(get_exercise_graph))
    .route("/workouts/log", post(create_workout_log))
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
            created_at TEXT NOT NULL
        );
        "#,
    )
    .execute(db)
    .await?;

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
        CREATE TABLE IF NOT EXISTS workout_logs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            profile_id INTEGER NOT NULL,
            exercise_id INTEGER NOT NULL,
            weight REAL NOT NULL,
            reps INTEGER NOT NULL,
            performed_at TEXT NOT NULL,
            FOREIGN KEY (profile_id) REFERENCES profiles(id),
            FOREIGN KEY (exercise_id) REFERENCES exercises(id)
        );
        "#,
    )
    .execute(db)
    .await?;

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
        INSERT INTO profiles (name, age, body_weight, created_at)
        VALUES (?, ?, ?, ?)
        "#,
    )
    .bind(&payload.name)
    .bind(payload.age)
    .bind(payload.body_weight)
    .bind(&now)
    .execute(&state.db)
    .await;

    match result {
        Ok(res) => {
            let id = res.last_insert_rowid();
            let profile = sqlx::query_as::<_, Profile>(
                r#"
                SELECT id, name, age, body_weight, created_at
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
        SELECT id, name, age, body_weight, created_at
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
) -> Json<ApiResponse<WorkoutLog>> {
    let now = Utc::now().to_rfc3339();

    let result = sqlx::query(
        r#"
        INSERT INTO workout_logs (profile_id, exercise_id, weight, reps, performed_at)
        VALUES (?, ?, ?, ?, ?)
        "#,
    )
    .bind(payload.profile_id)
    .bind(payload.exercise_id)
    .bind(payload.weight)
    .bind(payload.reps)
    .bind(&now)
    .execute(&state.db)
    .await;

    match result {
        Ok(res) => {
            let id = res.last_insert_rowid();

            let workout = sqlx::query_as::<_, WorkoutLog>(
                r#"
                SELECT id, profile_id, exercise_id, weight, reps, performed_at
                FROM workout_logs
                WHERE id = ?
                "#,
            )
            .bind(id)
            .fetch_one(&state.db)
            .await
            .unwrap();

            Json(ApiResponse {
                success: true,
                message: "Workout logged".to_string(),
                data: Some(workout),
            })
        }
        Err(e) => Json(ApiResponse {
            success: false,
            message: format!("Failed to log workout: {}", e),
            data: None,
        }),
    }
}

async fn get_exercise_history(
    State(state): State<AppState>,
    Path(exercise_id): Path<i64>,
) -> Json<ApiResponse<Vec<ExerciseHistoryItem>>> {
    let rows = sqlx::query_as::<_, WorkoutLog>(
        r#"
        SELECT id, profile_id, exercise_id, weight, reps, performed_at
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
            let history: Vec<ExerciseHistoryItem> = logs
                .into_iter()
                .map(|log| ExerciseHistoryItem {
                    date: log.performed_at,
                    weight: log.weight,
                    reps: log.reps,
                    volume: log.weight * log.reps as f64,
                })
                .collect();

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
            SUM(weight * reps) as total_volume
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
    let workout_days: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(DISTINCT substr(performed_at, 1, 10))
        FROM workout_logs
        WHERE profile_id = ?
        "#,
    )
    .bind(profile_id)
    .fetch_one(&state.db)
    .await
    .unwrap_or(Some(0))
    .unwrap_or(0);

    let total_volume: f64 = sqlx::query_scalar(
        r#"
        SELECT SUM(weight * reps)
        FROM workout_logs
        WHERE profile_id = ?
        "#,
    )
    .bind(profile_id)
    .fetch_one(&state.db)
    .await
    .unwrap_or(Some(0.0))
    .unwrap_or(0.0);

    let pr_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM (
            SELECT exercise_id, MAX(weight) AS max_weight
            FROM workout_logs
            WHERE profile_id = ?
            GROUP BY exercise_id
        )
        "#,
    )
    .bind(profile_id)
    .fetch_one(&state.db)
    .await
    .unwrap_or(Some(0))
    .unwrap_or(0);

    let consistency_points = workout_days;
    let strength_points = (total_volume / 100.0).floor() as i64;
    let pr_points = pr_count * 5;

    let score = consistency_points + strength_points + pr_points;
    let level = level_from_score(score);

    let summary = LevelSummary {
        profile_id,
        total_workout_days: workout_days,
        total_volume,
        pr_count,
        score,
        level,
    };

    Json(ApiResponse {
        success: true,
        message: "Level calculated".to_string(),
        data: Some(summary),
    })
}

fn level_from_score(score: i64) -> String {
    match score {
        0..=19 => "Beginner".to_string(),
        20..=49 => "Novice".to_string(),
        50..=99 => "Intermediate".to_string(),
        100..=179 => "Advanced".to_string(),
        180..=299 => "Elite".to_string(),
        _ => "Olympian".to_string(),
    }
}