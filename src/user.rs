use axum::extract::{State, Extension, Path, Json};
use crate::class::{AppState, ApiResponse, Profile, DashboardSummary, MuscleCoverageResponse, MissionSummary, ExerciseHistoryItem, ExerciseGraphPoint, Exercise};
use crate::exercise::{get_current_profile_by_user_id};
use crate::{get_dashboard,get_today_coverage, get_week_coverage};
use crate::exercise::{get_profile_exercises, get_owned_exercise, get_exercise_history, get_exercise_graph};
use crate::game::{get_missions};

pub async fn get_me_profile(
    State(state): State<AppState>,
    Extension(user_id): Extension<i64>,
) -> Json<ApiResponse<Profile>> {
    match get_current_profile_by_user_id(&state.db, user_id).await {
        Ok(profile) => Json(ApiResponse {
            success: true,
            message: "Profile fetched".to_string(),
            data: Some(profile),
        }),
        Err(e) => Json(ApiResponse {
            success: false,
            message: format!("Profile fetch failed: {}", e),
            data: None,
        }),
    }
}

pub async fn get_me_dashboard(
    State(state): State<AppState>,
    Extension(user_id): Extension<i64>,
) -> Json<ApiResponse<DashboardSummary>> {
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

    get_dashboard(State(state), Path(profile.id)).await
}

pub async fn get_me_exercises(
    State(state): State<AppState>,
    Extension(user_id): Extension<i64>,
) -> Json<ApiResponse<Vec<Exercise>>> {
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

    get_profile_exercises(State(state), Path(profile.id)).await
}


pub async fn get_me_today_coverage(
    State(state): State<AppState>,
    Extension(user_id): Extension<i64>,
) -> Json<ApiResponse<MuscleCoverageResponse>> {
    let profile = get_current_profile_by_user_id(&state.db, user_id).await.unwrap();
    get_today_coverage(State(state), Path(profile.id)).await
}

pub async fn get_me_week_coverage(
    State(state): State<AppState>,
    Extension(user_id): Extension<i64>,
) -> Json<ApiResponse<MuscleCoverageResponse>> {
    let profile = get_current_profile_by_user_id(&state.db, user_id).await.unwrap();
    get_week_coverage(State(state), Path(profile.id)).await
}

pub async fn get_me_missions(
    State(state): State<AppState>,
    Extension(user_id): Extension<i64>,
) -> Json<ApiResponse<MissionSummary>> {
    let profile = get_current_profile_by_user_id(&state.db, user_id).await.unwrap();
    get_missions(State(state), Path(profile.id)).await
}

pub async fn get_my_exercise_history(
    State(state): State<AppState>,
    Extension(user_id): Extension<i64>,
    Path(exercise_id): Path<i64>,
) -> Json<ApiResponse<Vec<ExerciseHistoryItem>>> {
    if get_owned_exercise(&state.db, user_id, exercise_id).await.is_err() {
        return Json(ApiResponse {
            success: false,
            message: "Exercise not found".to_string(),
            data: None,
        });
    }

    get_exercise_history(State(state), Path(exercise_id)).await
}


pub async fn get_my_exercise_graph(
    State(state): State<AppState>,
    Extension(user_id): Extension<i64>,
    Path(exercise_id): Path<i64>,
) -> Json<ApiResponse<Vec<ExerciseGraphPoint>>> {
    if get_owned_exercise(&state.db, user_id, exercise_id).await.is_err() {
        return Json(ApiResponse {
            success: false,
            message: "Exercise not found".to_string(),
            data: None,
        });
    }

    get_exercise_graph(State(state), Path(exercise_id)).await
}

