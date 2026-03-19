use axum::{
    extract::{Path, State},
    http::Method,
    routing::{get, post},
    Json, Router,
};
use chrono::{DateTime, Duration, NaiveDate, Utc};
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
        .route("/profiles/{id}/exercises", get(get_profile_exercises))
        .route("/profiles/{id}/level", get(get_profile_level))
        .route("/profiles/{id}/dashboard", get(get_dashboard))
        .route("/profiles/{id}/badges", get(get_badges))
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
            xp INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL
        );
        "#,
    )
    .execute(db)
    .await?;

    // Safe migration in case old DB already exists without xp column
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

            let volume = payload.weight * payload.reps as f64;
            let volume_xp = (volume / 10.0).floor() as i64;
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
            let mut running_max = 0.0_f64;
            let mut history = Vec::new();

            for log in logs {
                let is_pr = log.weight > running_max;
                if log.weight > running_max {
                    running_max = log.weight;
                }

                history.push(ExerciseHistoryItem {
                    date: log.performed_at,
                    weight: log.weight,
                    reps: log.reps,
                    volume: log.weight * log.reps as f64,
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

async fn build_level_summary(db: &SqlitePool, profile_id: i64) -> Result<LevelSummary, sqlx::Error> {
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
        SELECT SUM(weight * reps)
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
        ORDER BY created_at DESC
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