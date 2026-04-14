use axum::extract::{State, Json};
use crate::class::{AppState, ApiResponse, ExerciseCatalogItem};

pub async fn search_catalog_exercises(
    State(state): State<AppState>,
) -> Json<ApiResponse<Vec<ExerciseCatalogItem>>> {
    let result = sqlx::query_as::<_, ExerciseCatalogItem>(
        r#"
        SELECT id, name, muscle_group, equipment, secondary_muscles
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


pub fn standard_muscle_groups() -> Vec<String> {
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