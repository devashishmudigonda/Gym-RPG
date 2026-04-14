mod auth;
mod auth_middleware;
mod exercise;
mod workout;
mod class;
mod user;
mod game;
mod exercise_data;
mod profile_dashboard;
mod calculation;
mod leaderboard;





use auth::{*};
use exercise::{*};
use workout::{*};
use class::{*};
use user::{*};
use exercise_data::{*};
use game::{*};
use profile_dashboard::{*};
use calculation::{*};
use leaderboard::{*};
use auth_middleware::require_auth;
use axum::{
    extract::{Path, State},
    http::Method,
    middleware,
    routing::{get, post},
    Json, Router,
};
use chrono::{Duration, Utc};
use sqlx::sqlite::SqlitePoolOptions;
use std::net::SocketAddr;
use tower_http::cors::{Any, CorsLayer};


#[tokio::main]
async fn main() {
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite://gym_app.db".to_string());

    let jwt_secret = std::env::var("JWT_SECRET")
        .unwrap_or_else(|_| {
            eprintln!("WARNING: JWT_SECRET not set, using insecure default. Set JWT_SECRET in production!");
            "dev-only-change-this-in-production".to_string()
        })
        .into_bytes();

    let db = SqlitePoolOptions::new()
        .max_connections(4)
        .connect(&database_url)
        .await
        .expect("Failed to connect to SQLite");

    init_db(&db).await.expect("Failed to initialize DB");

    let state = AppState { db, jwt_secret };

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST])
        .allow_headers(Any);

    let protected = Router::new()
        .route("/me/profile", get(get_me_profile))
        .route("/me/dashboard", get(get_me_dashboard))
        .route("/me/exercises", get(get_me_exercises))
        .route("/me/coverage/today", get(get_me_today_coverage))
        .route("/me/coverage/week", get(get_me_week_coverage))
        .route("/me/missions", get(get_me_missions))
        .route("/me/workouts/active", get(get_my_active_workout))
        .route("/me/workout-days", get(get_my_workout_days))
        .route("/me/exercises/{id}/history", get(get_my_exercise_history))
        .route("/me/exercises/{id}/graph", get(get_my_exercise_graph))
        .route("/exercises", post(create_exercise))
        .route("/exercises/{id}", post(update_exercise))
        .route("/exercises/{id}/delete", post(delete_exercise))
        .route("/workouts/start", post(start_workout_session))
        .route("/workouts/end/{session_id}", post(end_workout_session))
        .route("/workouts/log", post(create_workout_log))
        .route("/workouts/{id}", post(update_workout_log))
        .route("/workouts/{id}/delete", post(delete_workout_log))
        .route("/leaderboard", get(get_leaderboard))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            require_auth,
        ));

    let app = Router::new()
        .route("/", get(health_check))
        .route("/auth/register", post(register))
        .route("/auth/login", post(login))
        .route("/catalog/exercises", get(search_catalog_exercises))
        .merge(protected)
        .with_state(state)
        .layer(cors);

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3000);

    let addr: SocketAddr = format!("0.0.0.0:{port}").parse().unwrap();
    println!("Server running at http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn init_db(db: &sqlx::SqlitePool) -> Result<(), sqlx::Error> {
    // Enable WAL mode for better concurrent read/write performance and crash safety
    sqlx::query("PRAGMA journal_mode=WAL;").execute(db).await?;
    sqlx::query("PRAGMA synchronous=NORMAL;").execute(db).await?;
    sqlx::query("PRAGMA busy_timeout=5000;").execute(db).await?;
    sqlx::query("PRAGMA foreign_keys=ON;").execute(db).await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS users (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            email TEXT NOT NULL UNIQUE,
            password_hash TEXT NOT NULL,
            created_at TEXT NOT NULL
        );
        "#,
    )
    .execute(db)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS profiles (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL UNIQUE,
            name TEXT NOT NULL,
            age INTEGER NOT NULL,
            body_weight REAL NOT NULL,
            xp INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL,
            FOREIGN KEY (user_id) REFERENCES users(id)
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
            equipment TEXT NOT NULL DEFAULT '',
            secondary_muscles TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL,
            FOREIGN KEY (profile_id) REFERENCES profiles(id)
        );
        "#,
    )
    .execute(db)
    .await?;

    // Migrations for existing DBs
    let _ = sqlx::query("ALTER TABLE exercises ADD COLUMN equipment TEXT NOT NULL DEFAULT ''")
        .execute(db).await;
    let _ = sqlx::query("ALTER TABLE exercises ADD COLUMN secondary_muscles TEXT NOT NULL DEFAULT ''")
        .execute(db).await;

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
            muscle_group TEXT NOT NULL,
            equipment TEXT NOT NULL DEFAULT '',
            secondary_muscles TEXT NOT NULL DEFAULT ''
        );
        "#,
    )
    .execute(db)
    .await?;

    let _ = sqlx::query("ALTER TABLE exercise_catalog ADD COLUMN equipment TEXT NOT NULL DEFAULT ''")
        .execute(db).await;
    let _ = sqlx::query("ALTER TABLE exercise_catalog ADD COLUMN secondary_muscles TEXT NOT NULL DEFAULT ''")
        .execute(db).await;

    // name, muscle_group, equipment, secondary_muscles
    let default_exercises: Vec<(&str, &str, &str, &str)> = vec![
        ("Bench Press",          "Chest",      "Barbell",    "Triceps, Shoulders"),
        ("Incline Bench Press",  "Chest",      "Barbell",    "Triceps, Shoulders"),
        ("Push Up",              "Chest",      "Bodyweight", "Triceps, Core"),
        ("Chest Fly",            "Chest",      "Dumbbell",   "Shoulders"),
        ("Squat",                "Legs",       "Barbell",    "Glutes, Core"),
        ("Leg Press",            "Legs",       "Machine",    "Glutes"),
        ("Lunge",                "Legs",       "Dumbbell",   "Glutes, Core"),
        ("Romanian Deadlift",    "Hamstrings", "Barbell",    "Glutes, Back"),
        ("Deadlift",             "Back",       "Barbell",    "Legs, Glutes, Core"),
        ("Pull Up",              "Back",       "Bodyweight", "Biceps"),
        ("Lat Pulldown",         "Back",       "Cable",      "Biceps"),
        ("Barbell Row",          "Back",       "Barbell",    "Biceps, Core"),
        ("Shoulder Press",       "Shoulders",  "Barbell",    "Triceps"),
        ("Lateral Raise",        "Shoulders",  "Dumbbell",   ""),
        ("Rear Delt Fly",        "Shoulders",  "Dumbbell",   "Back"),
        ("Barbell Curl",         "Biceps",     "Barbell",    "Forearms"),
        ("Hammer Curl",          "Biceps",     "Dumbbell",   "Forearms"),
        ("Tricep Pushdown",      "Triceps",    "Cable",      ""),
        ("Skull Crusher",        "Triceps",    "EZ Bar",     ""),
        ("Plank",                "Core",       "Bodyweight", "Shoulders, Back"),
        ("Crunch",               "Core",       "Bodyweight", ""),
        ("Leg Raise",            "Core",       "Bodyweight", ""),
        ("Calf Raise",           "Calves",     "Machine",    ""),
        ("Hip Thrust",           "Glutes",     "Barbell",    "Hamstrings"),
    ];

    for (name, muscle_group, equipment, secondary) in &default_exercises {
        let _ = sqlx::query(
            r#"
            INSERT INTO exercise_catalog (name, muscle_group, equipment, secondary_muscles)
            VALUES (?, ?, ?, ?)
            ON CONFLICT(name) DO UPDATE SET
                equipment = excluded.equipment,
                secondary_muscles = excluded.secondary_muscles
            "#,
        )
        .bind(name)
        .bind(muscle_group)
        .bind(equipment)
        .bind(secondary)
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




