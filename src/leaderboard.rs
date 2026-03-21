use axum::extract::{State, Json};
use crate::class::{AppState, ApiResponse};

pub async fn get_leaderboard(
    State(state): State<AppState>,
) -> Json<ApiResponse<Vec<(String, i64)>>> {
    let result = sqlx::query_as(
        r#"
        SELECT name, xp
        FROM profiles
        ORDER BY xp DESC
        LIMIT 10
        "#,
    )
    .fetch_all(&state.db)
    .await;

    match result {
        Ok(leaderboard) => Json(ApiResponse {
            success: true,
            message: "Leaderboard fetched".to_string(),
            data: Some(leaderboard),
        }),
        Err(e) => Json(ApiResponse {
            success: false,
            message: format!("Failed to fetch leaderboard: {}", e),
            data: None,
        }),
    }
}
