use crate::class::{AppState, ApiResponse, LoginRequest, RegisterRequest, AuthResponse,Profile,User};
use axum::extract::{State, Json};
use argon2::{
    password_hash::{
        rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString,
    },
    Argon2,
};
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

const JWT_SECRET: &[u8] = b"CHANGE_THIS_TO_A_LONG_RANDOM_SECRET";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: i64,
    pub exp: usize,
}

pub fn hash_password(password: &str) -> Result<String, String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|e| e.to_string())
}

pub fn verify_password(password: &str, password_hash: &str) -> Result<bool, String> {
    let parsed_hash = PasswordHash::new(password_hash).map_err(|e| e.to_string())?;
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok())
}

pub fn create_jwt(user_id: i64) -> Result<String, String> {
    let exp = (Utc::now() + Duration::days(7)).timestamp() as usize;
    let claims = Claims { sub: user_id, exp };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(JWT_SECRET),
    )
    .map_err(|e| e.to_string())
}

pub fn verify_jwt(token: &str) -> Result<Claims, String> {
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(JWT_SECRET),
        &Validation::default(),
    )
    .map(|data| data.claims)
    .map_err(|e| e.to_string())
}

pub async fn register(
    State(state): State<AppState>,
    Json(payload): Json<RegisterRequest>,
) -> Json<ApiResponse<AuthResponse>> {
    let now = Utc::now().to_rfc3339();

    let password_hash = match hash_password(&payload.password) {
        Ok(v) => v,
        Err(e) => {
            return Json(ApiResponse {
                success: false,
                message: format!("Password hash failed: {}", e),
                data: None,
            })
        }
    };

    let user_insert = sqlx::query(
        r#"
        INSERT INTO users (email, password_hash, created_at)
        VALUES (?, ?, ?)
        "#,
    )
    .bind(&payload.email)
    .bind(&password_hash)
    .bind(&now)
    .execute(&state.db)
    .await;

    let user_id = match user_insert {
        Ok(res) => res.last_insert_rowid(),
        Err(e) => {
            return Json(ApiResponse {
                success: false,
                message: format!("Failed to create user: {}", e),
                data: None,
            })
        }
    };

    let profile_insert = sqlx::query(
        r#"
        INSERT INTO profiles (user_id, name, age, body_weight, xp, created_at)
        VALUES (?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(user_id)
    .bind(&payload.name)
    .bind(payload.age)
    .bind(payload.body_weight)
    .bind(0_i64)
    .bind(&now)
    .execute(&state.db)
    .await;

    if let Err(e) = profile_insert {
        return Json(ApiResponse {
            success: false,
            message: format!("Failed to create profile: {}", e),
            data: None,
        });
    }

    let profile = sqlx::query_as::<_, Profile>(
        r#"
        SELECT id, name, age, body_weight, xp, created_at
        FROM profiles
        WHERE user_id = ?
        "#,
    )
    .bind(user_id)
    .fetch_one(&state.db)
    .await
    .unwrap();

    let token = match create_jwt(user_id) {
        Ok(t) => t,
        Err(e) => {
            return Json(ApiResponse {
                success: false,
                message: format!("Token creation failed: {}", e),
                data: None,
            })
        }
    };

    Json(ApiResponse {
        success: true,
        message: "Registered successfully".to_string(),
        data: Some(AuthResponse { token, profile }),
    })
}

pub async fn login(
    State(state): State<AppState>,
    Json(payload): Json<LoginRequest>,
) -> Json<ApiResponse<AuthResponse>> {
    let user = sqlx::query_as::<_, User>(
        r#"
        SELECT id, email, password_hash, created_at
        FROM users
        WHERE email = ?
        "#,
    )
    .bind(&payload.email)
    .fetch_one(&state.db)
    .await;

    let user = match user {
        Ok(u) => u,
        Err(_) => {
            return Json(ApiResponse {
                success: false,
                message: "Invalid credentials".to_string(),
                data: None,
            })
        }
    };

    let valid = verify_password(&payload.password, &user.password_hash).unwrap_or(false);
    if !valid {
        return Json(ApiResponse {
            success: false,
            message: "Invalid credentials".to_string(),
            data: None,
        });
    }

    let profile = sqlx::query_as::<_, Profile>(
        r#"
        SELECT id, name, age, body_weight, xp, created_at
        FROM profiles
        WHERE user_id = ?
        "#,
    )
    .bind(user.id)
    .fetch_one(&state.db)
    .await
    .unwrap();

    let token = match create_jwt(user.id) {
        Ok(t) => t,
        Err(e) => {
            return Json(ApiResponse {
                success: false,
                message: format!("Token creation failed: {}", e),
                data: None,
            })
        }
    };

    Json(ApiResponse {
        success: true,
        message: "Login successful".to_string(),
        data: Some(AuthResponse { token, profile }),
    })
}
