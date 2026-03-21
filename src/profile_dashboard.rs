use axum::extract::{State, Path, Json};
use crate::class::{AppState, ApiResponse, DashboardSummary, LevelSummary, Badge, Profile};
use crate::game::{build_level_summary};

pub async fn get_dashboard(
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


pub async fn get_profile_level(
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

pub async fn get_badges(
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
