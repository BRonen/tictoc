use axum::{
    routing::{get, post},
    Router,
    extract::{State, Json},
};
use serde::{Deserialize, Serialize};
use jsonwebtoken::{encode, Header, EncodingKey};
use bcrypt::{hash, verify};
use sqlx::PgPool;
use dotenv::dotenv;
use std::env;

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
struct User {
    id: i32,
    name: String,
    email: String,
    password_hash: String,
}

#[derive(Clone)]
struct AppState {
    pool: PgPool,
}

#[derive(Deserialize)]
struct CreateUserRequest {
    name: String,
    email: String,
    password: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct CreateUserResponse {
    id: i32,
    name: String,
    email: String,
}

#[derive(Deserialize)]
struct LoginUserRequest {
    email: String,
    password: String,
}

#[derive(Serialize, Deserialize)]
struct LoginUserResponse {
    token: String,
}

async fn read_user(State(state): State<AppState>) -> String {
    let users = sqlx::query_as!(
        CreateUserResponse,
        "SELECT id, name, email FROM users ORDER BY id"
    )
    .fetch_all(&state.pool)
    .await
    .unwrap();

    serde_json::to_string(&users).unwrap()
}

async fn create_user(
    State(state): State<AppState>,
    Json(payload): Json<CreateUserRequest>,
) -> String {
    let password_hash = hash(payload.password, 10).unwrap();

    let user = sqlx::query_as!(
        CreateUserResponse,
        "INSERT INTO users (name, email, password_hash) VALUES ($1, $2, $3) RETURNING id, name, email",
        payload.name,
        payload.email,
        password_hash
    )
    .fetch_one(&state.pool)
    .await
    .unwrap();

    serde_json::to_string(&user).unwrap()
}

async fn login(
    State(state): State<AppState>,
    Json(payload): Json<LoginUserRequest>,
) -> String {
    let user = sqlx::query!(
        "SELECT id, name, email, password_hash FROM users WHERE email = $1",
        payload.email
    )
    .fetch_optional(&state.pool)
    .await
    .unwrap();

    match user {
        Some(user) => {
            if !verify(&payload.password, &user.password_hash).unwrap() {
                return "Invalid password".to_string();
            }

            let user_data = CreateUserResponse {
                id: user.id,
                name: user.name,
                email: user.email,
            };

            let token = LoginUserResponse {
                token: encode(
                    &Header::default(),
                    &user_data,
                    &EncodingKey::from_secret("secret".as_ref()),
                ).unwrap(),
            };

            serde_json::to_string(&token).unwrap()
        }
        _ => "User not found".to_string(),
    }
}

#[tokio::main]
async fn main() {
    dotenv().ok();
    let pool = PgPool::connect(&env::var("DATABASE_URL").unwrap())
        .await
        .unwrap();

    sqlx::migrate!()
        .run(&pool)
        .await
        .unwrap();

    let state = AppState { pool };

    let app = Router::new()
        .route("/users", get(read_user))
        .route("/users/create", post(create_user))
        .route("/users/login", post(login))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .unwrap();
    println!("Server running on http://0.0.0.0:3000");
    axum::serve(listener, app).await.unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;
    use jsonwebtoken::{decode, DecodingKey, Validation};
    use std::collections::HashSet;
    use uuid;

    async fn setup_test_db(db_name: &str) -> PgPool {
        dotenv().ok();
        let base_url = env::var("DATABASE_URL").unwrap();
        
        let admin_pool = PgPool::connect(&base_url)
            .await
            .unwrap();

        sqlx::query(&format!("CREATE DATABASE {}", db_name))
            .execute(&admin_pool)
            .await
            .unwrap();

        let test_url = base_url.replace("/tictoc", &format!("/{}", db_name));
        let pool = PgPool::connect(&test_url)
            .await
            .unwrap();

        sqlx::migrate!()
            .run(&pool)
            .await
            .unwrap();

        pool
    }

    async fn cleanup_test_db(db_name: &str) {
        let base_url = env::var("DATABASE_URL").unwrap();
        let admin_pool = PgPool::connect(&base_url)
            .await
            .unwrap();

        sqlx::query(&format!("DROP DATABASE IF EXISTS {}", db_name))
            .execute(&admin_pool)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_create_user() {
        let db_name = format!("test_{}", uuid::Uuid::new_v4().simple());
        let pool = setup_test_db(&db_name).await;
        let state = AppState { pool };

        let user = CreateUserRequest {
            name: "Chad".to_string(),
            email: "chad1@gmail.com".to_string(),
            password: "password".to_string()
        };

        let response = create_user(
            State(state.clone()),
            Json(user)
        ).await;
        assert_eq!(response, "{\"id\":1,\"name\":\"Chad\",\"email\":\"chad1@gmail.com\"}");

        let user = CreateUserRequest {
            name: "User".to_string(),
            email: "user@gmail.com".to_string(),
            password: "password".to_string()
        };

        let response = create_user(
            State(state),
            Json(user)
        ).await;
        assert_eq!(response, "{\"id\":2,\"name\":\"User\",\"email\":\"user@gmail.com\"}");

        cleanup_test_db(&db_name).await;
    }

    #[tokio::test]
    async fn test_login() {
        let db_name = format!("test_{}", uuid::Uuid::new_v4().simple());
        let pool = setup_test_db(&db_name).await;
        let state = AppState { pool };

        let user = CreateUserRequest {
            name: "Chad".to_string(),
            email: "chad2@gmail.com".to_string(),
            password: "password".to_string()
        };

        create_user(State(state.clone()), Json(user)).await;

        let login_user = LoginUserRequest {
            email: "chad2@gmail.com".to_string(),
            password: "password".to_string()
        };

        let response = login(State(state), Json(login_user)).await;

        let token_response: LoginUserResponse = serde_json::from_str(&response).unwrap();
        
        let mut validation = Validation::default();
        validation.validate_exp = false;
        validation.required_spec_claims = HashSet::new();

        let token_data = decode::<CreateUserResponse>(
            &token_response.token,
            &DecodingKey::from_secret("secret".as_ref()),
            &validation,
        ).unwrap();

        assert_eq!(token_data.claims.id, 1);
        assert_eq!(token_data.claims.name, "Chad");
        assert_eq!(token_data.claims.email, "chad2@gmail.com");

        cleanup_test_db(&db_name).await;
    }
}