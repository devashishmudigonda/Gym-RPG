use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};





#[derive(Clone)]
 pub struct AppState {
    pub db: SqlitePool,
    pub jwt_secret: Vec<u8>,
}

#[derive(Serialize)]
 pub struct ApiResponse<T> {
    pub success: bool,
    pub message: String,
    pub data: Option<T>,
}

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct User {
    pub id: i64,
    pub email: String,
    pub password_hash: String,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct Profile {
    pub id: i64,
    pub name: String,
    pub age: i64,
    pub body_weight: f64,
    pub xp: i64,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RegisterRequest {
    pub email: String,
    pub password: String,
    pub name: String,
    pub age: i64,
    pub body_weight: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub token: String,
    pub profile: Profile,
}

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct Exercise {
    pub id: i64,
    pub profile_id: i64,
    pub name: String,
    pub muscle_group: String,
    pub equipment: String,
    pub secondary_muscles: String,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateExercise {
    pub profile_id: i64,
    pub name: String,
    pub muscle_group: String,
    #[serde(default)]
    pub equipment: String,
    #[serde(default)]
    pub secondary_muscles: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateExercise {
    pub name: String,
    pub muscle_group: String,
    #[serde(default)]
    pub equipment: String,
    #[serde(default)]
    pub secondary_muscles: String,
}

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct WorkoutLog {
    pub id: i64,
    pub profile_id: i64,
    pub exercise_id: i64,
    pub session_id: i64,
    pub weight: f64,
    pub reps: i64,
    pub total_volume: f64,
    pub performed_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateWorkoutLog {
    pub profile_id: i64,
    pub exercise_id: i64,
    pub session_id: i64,
    pub weight: f64,
    pub reps: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateWorkoutLog {
    pub weight: f64,
    pub reps: i64,
}

#[derive(Debug, Serialize)]
pub struct WorkoutLogResponse {
    pub workout: WorkoutLog,
    pub gained_xp: i64,
    pub total_xp: i64,
    pub new_pr: bool,
    pub unlocked_badges: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ExerciseHistoryItem {
    pub id: i64,
    pub date: String,
    pub weight: f64,
    pub reps: i64,
    pub volume: f64,
    pub is_pr: bool,
}

#[derive(Debug, Serialize)]
pub struct ExerciseGraphPoint {
    pub date: String,
    pub max_weight: f64,
    pub total_volume: f64,
}

#[derive(Debug, Serialize)]
pub struct LevelSummary {
    pub profile_id: i64,
    pub total_workout_days: i64,
    pub total_volume: f64,
    pub current_streak: i64,
    pub longest_streak: i64,
    pub pr_count: i64,
    pub xp: i64,
    pub score: i64,
    pub level: String,
}

#[derive(Debug, Serialize, FromRow)]
pub struct Badge {
    pub id: i64,
    pub profile_id: i64,
    pub name: String,
    pub description: String,
    pub unlocked_at: String,
}

#[derive(Debug, Serialize)]
pub struct DashboardSummary {
    pub profile_id: i64,
    pub name: String,
    pub xp: i64,
    pub level: String,
    pub score: i64,
    pub total_workout_days: i64,
    pub current_streak: i64,
    pub longest_streak: i64,
    pub pr_count: i64,
    pub total_volume: f64,
    pub badges: Vec<Badge>,
}

#[derive(Debug, Serialize, Deserialize, FromRow, Clone)]
pub struct ExerciseCatalogItem {
    pub id: i64,
    pub name: String,
    pub muscle_group: String,
    pub equipment: String,
    pub secondary_muscles: String,
}

#[derive(Debug, Serialize)]
pub struct MuscleCoverageResponse {
    pub covered: Vec<String>,
    pub missing: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct Mission {
    pub name: String,
    pub description: String,
    pub completed: bool,
}

#[derive(Debug, Serialize)]
pub struct MissionSummary {
    pub profile_id: i64,
    pub current_streak: i64,
    pub weekly_missions: Vec<Mission>,
}

#[derive(Debug, Serialize, Deserialize, FromRow, Clone)]
pub struct WorkoutSession {
    pub id: i64,
    pub profile_id: i64,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub status: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StartWorkoutSessionRequest {
    pub profile_id: i64,
}

#[derive(Debug, Serialize)]
pub struct ActiveWorkoutResponse {
    pub session: WorkoutSession,
    pub entries: Vec<WorkoutLogWithExercise>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct WorkoutLogWithExercise {
    pub id: i64,
    pub profile_id: i64,
    pub exercise_id: i64,
    pub session_id: i64,
    pub exercise_name: String,
    pub muscle_group: String,
    pub weight: f64,
    pub reps: i64,
    pub total_volume: f64,
    pub performed_at: String,
}