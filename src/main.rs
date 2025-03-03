use axum::{
    routing::{get, post},
    Router,
    extract::{State, Json},
};
use serde::{Deserialize, Serialize};
use jsonwebtoken::{encode, Header, EncodingKey};
use bcrypt::{hash, verify};
use sqlx::PgPool;

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
        None => "User not found".to_string(),
    }
}

#[tokio::main]
async fn main() {
    // Create a connection pool
    let pool = PgPool::connect("postgres://postgres:postgres@localhost/tictoc")
        .await
        .unwrap();

    // Run migrations
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
    use jsonwebtoken::{decode, DecodingKey, Validation, Algorithm};
    use std::collections::HashSet;

    async fn setup_test_db() -> PgPool {
        let pool = PgPool::connect("postgres://postgres:postgres@localhost/tictoc_test")
            .await
            .unwrap();

        // Run migrations
        sqlx::migrate!()
            .run(&pool)
            .await
            .unwrap();

        // Clear the database
        sqlx::query!("TRUNCATE TABLE users")
            .execute(&pool)
            .await
            .unwrap();

        pool
    }

    #[tokio::test]
    async fn test_create_user() {
        let pool = setup_test_db().await;
        let state = AppState { pool };

        let user = CreateUserRequest {
            name: "Chad".to_string(),
            email: "chad@gmail.com".to_string(),
            password: "password".to_string()
        };

        let response = create_user(
            State(state.clone()),
            Json(user)
        ).await;
        assert_eq!(response, "{\"id\":1,\"name\":\"Chad\",\"email\":\"chad@gmail.com\"}");

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
    }

    #[tokio::test]
    async fn test_read_user() {
        let pool = setup_test_db().await;
        let state = AppState { pool };

        let response = read_user(State(state.clone())).await;
        assert_eq!(response, "[]");

        let user = CreateUserRequest {
            name: "Chad".to_string(),
            email: "chad@gmail.com".to_string(),
            password: "password".to_string()
        };

        let response = create_user(
            State(state.clone()),
            Json(user)
        ).await;
        assert_eq!(response, "{\"id\":1,\"name\":\"Chad\",\"email\":\"chad@gmail.com\"}");

        let response = read_user(State(state.clone())).await;
        assert_eq!(response, "[{\"id\":1,\"name\":\"Chad\",\"email\":\"chad@gmail.com\"}]");

        let user = CreateUserRequest {
            name: "User".to_string(),
            email: "user@gmail.com".to_string(),
            password: "password".to_string()
        };

        let response = create_user(
            State(state.clone()),
            Json(user)
        ).await;
        assert_eq!(response, "{\"id\":2,\"name\":\"User\",\"email\":\"user@gmail.com\"}");

        let response = read_user(State(state)).await;
        assert_eq!(response, "[{\"id\":1,\"name\":\"Chad\",\"email\":\"chad@gmail.com\"},{\"id\":2,\"name\":\"User\",\"email\":\"user@gmail.com\"}]");
    }

    #[tokio::test]
    async fn test_login() {
        let pool = setup_test_db().await;
        let state = AppState { pool };

        let user = CreateUserRequest {
            name: "Chad".to_string(),
            email: "chad@gmail.com".to_string(),
            password: "password".to_string()
        };

        let response = create_user(
            State(state.clone()),
            Json(user)
        ).await;
        assert_eq!(response, "{\"id\":1,\"name\":\"Chad\",\"email\":\"chad@gmail.com\"}");

        let login_request = LoginUserRequest {
            email: "chad@gmail.com".to_string(),
            password: "password".to_string()
        };
        
        let response = login(
            State(state.clone()),
            Json(login_request)
        ).await;
        
        assert!(response.contains("token"));
        let login_response: LoginUserResponse = serde_json::from_str(&response).unwrap();
        assert!(login_response.token.len() > 0);
        println!("token: {}", login_response.token);

        let mut validation = Validation::new(Algorithm::HS256);
        validation.validate_exp = false;
        validation.required_spec_claims = HashSet::new();

        let decoded_user = decode::<CreateUserResponse>(&login_response.token, &DecodingKey::from_secret("secret".as_ref()), &validation).unwrap();
        let user = CreateUserResponse { id: 1, name: "Chad".to_string(), email: "chad@gmail.com".to_string() };
        assert_eq!(decoded_user.claims, user);
    }
}