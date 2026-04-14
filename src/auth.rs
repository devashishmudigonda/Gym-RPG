use crate::class::{AppState, ApiResponse, LoginRequest, RegisterRequest, AuthResponse, Profile, User};
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

pub fn create_jwt(user_id: i64, secret: &[u8]) -> Result<String, String> {
    let exp = (Utc::now() + Duration::days(7)).timestamp() as usize;
    let claims = Claims { sub: user_id, exp };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret),
    )
    .map_err(|e| e.to_string())
}

pub fn verify_jwt(token: &str, secret: &[u8]) -> Result<Claims, String> {
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret),
        &Validation::default(),
    )
    .map(|data| data.claims)
    .map_err(|e| e.to_string())
}

fn is_valid_email(email: &str) -> bool {
    let parts: Vec<&str> = email.split('@').collect();
    if parts.len() != 2 {
        return false;
    }
    let local = parts[0];
    let domain = parts[1];
    !local.is_empty()
        && domain.contains('.')
        && domain.len() > 2
        && !domain.starts_with('.')
        && !domain.ends_with('.')
}

pub async fn register(
    State(state): State<AppState>,
    Json(payload): Json<RegisterRequest>,
) -> Json<ApiResponse<AuthResponse>> {
    // --- Input validation ---
    let email = payload.email.trim().to_lowercase();
    if email.is_empty() {
        return Json(ApiResponse {
            success: false,
            message: "Email is required".to_string(),
            data: None,
        });
    }
    if email.len() > 254 {
        return Json(ApiResponse {
            success: false,
            message: "Email is too long".to_string(),
            data: None,
        });
    }
    if !is_valid_email(&email) {
        return Json(ApiResponse {
            success: false,
            message: "Invalid email format".to_string(),
            data: None,
        });
    }

    if payload.password.is_empty() {
        return Json(ApiResponse {
            success: false,
            message: "Password is required".to_string(),
            data: None,
        });
    }
    if payload.password.len() < 8 {
        return Json(ApiResponse {
            success: false,
            message: "Password must be at least 8 characters".to_string(),
            data: None,
        });
    }
    if payload.password.len() > 128 {
        return Json(ApiResponse {
            success: false,
            message: "Password must be 128 characters or less".to_string(),
            data: None,
        });
    }

    let name = payload.name.trim().to_string();
    if name.is_empty() {
        return Json(ApiResponse {
            success: false,
            message: "Name is required".to_string(),
            data: None,
        });
    }
    if name.len() > 50 {
        return Json(ApiResponse {
            success: false,
            message: "Name must be 50 characters or less".to_string(),
            data: None,
        });
    }

    if payload.age < 13 || payload.age > 120 {
        return Json(ApiResponse {
            success: false,
            message: "Age must be between 13 and 120".to_string(),
            data: None,
        });
    }

    if payload.body_weight < 20.0 || payload.body_weight > 500.0 {
        return Json(ApiResponse {
            success: false,
            message: "Body weight must be between 20 and 500 kg".to_string(),
            data: None,
        });
    }

    // --- Hash password ---
    let now = Utc::now().to_rfc3339();

    let password_hash = match hash_password(&payload.password) {
        Ok(v) => v,
        Err(_) => {
            return Json(ApiResponse {
                success: false,
                message: "Failed to process registration. Please try again.".to_string(),
                data: None,
            })
        }
    };

    // --- Insert user ---
    let user_insert = sqlx::query(
        r#"
        INSERT INTO users (email, password_hash, created_at)
        VALUES (?, ?, ?)
        "#,
    )
    .bind(&email)
    .bind(&password_hash)
    .bind(&now)
    .execute(&state.db)
    .await;

    let user_id = match user_insert {
        Ok(res) => res.last_insert_rowid(),
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("UNIQUE constraint failed") {
                return Json(ApiResponse {
                    success: false,
                    message: "An account with this email already exists".to_string(),
                    data: None,
                });
            }
            return Json(ApiResponse {
                success: false,
                message: "Failed to create account. Please try again.".to_string(),
                data: None,
            });
        }
    };

    // --- Insert profile ---
    let profile_insert = sqlx::query(
        r#"
        INSERT INTO profiles (user_id, name, age, body_weight, xp, created_at)
        VALUES (?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(user_id)
    .bind(&name)
    .bind(payload.age)
    .bind(payload.body_weight)
    .bind(0_i64)
    .bind(&now)
    .execute(&state.db)
    .await;

    if let Err(_) = profile_insert {
        return Json(ApiResponse {
            success: false,
            message: "Failed to create profile. Please try again.".to_string(),
            data: None,
        });
    }

    // --- Fetch created profile ---
    let profile = match sqlx::query_as::<_, Profile>(
        r#"
        SELECT id, name, age, body_weight, xp, created_at
        FROM profiles
        WHERE user_id = ?
        "#,
    )
    .bind(user_id)
    .fetch_one(&state.db)
    .await
    {
        Ok(p) => p,
        Err(_) => {
            return Json(ApiResponse {
                success: false,
                message: "Account created but failed to load profile. Please log in.".to_string(),
                data: None,
            })
        }
    };

    let token = match create_jwt(user_id, &state.jwt_secret) {
        Ok(t) => t,
        Err(_) => {
            return Json(ApiResponse {
                success: false,
                message: "Account created but failed to generate session. Please log in.".to_string(),
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
    // --- Input validation (keep errors generic to prevent user enumeration) ---
    let email = payload.email.trim().to_lowercase();
    if email.is_empty() || payload.password.is_empty() {
        return Json(ApiResponse {
            success: false,
            message: "Invalid credentials".to_string(),
            data: None,
        });
    }
    // Reject excessively long passwords before Argon2 (DoS protection)
    if payload.password.len() > 128 {
        return Json(ApiResponse {
            success: false,
            message: "Invalid credentials".to_string(),
            data: None,
        });
    }

    // --- Fetch user ---
    let user = sqlx::query_as::<_, User>(
        r#"
        SELECT id, email, password_hash, created_at
        FROM users
        WHERE email = ?
        "#,
    )
    .bind(&email)
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

    // --- Verify password ---
    let valid = verify_password(&payload.password, &user.password_hash).unwrap_or(false);
    if !valid {
        return Json(ApiResponse {
            success: false,
            message: "Invalid credentials".to_string(),
            data: None,
        });
    }

    // --- Fetch profile ---
    let profile = match sqlx::query_as::<_, Profile>(
        r#"
        SELECT id, name, age, body_weight, xp, created_at
        FROM profiles
        WHERE user_id = ?
        "#,
    )
    .bind(user.id)
    .fetch_one(&state.db)
    .await
    {
        Ok(p) => p,
        Err(_) => {
            return Json(ApiResponse {
                success: false,
                message: "Login failed: profile not found. Please contact support.".to_string(),
                data: None,
            })
        }
    };

    let token = match create_jwt(user.id, &state.jwt_secret) {
        Ok(t) => t,
        Err(_) => {
            return Json(ApiResponse {
                success: false,
                message: "Failed to generate session. Please try again.".to_string(),
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
