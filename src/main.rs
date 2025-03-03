use axum::{
    routing::{get, post},
    Router,
    extract::{State, Json},
};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Clone)]
struct User {
    id: i32,
    name: String,
}

#[derive(Clone)]
struct AppState {
    users: Arc<Mutex<HashMap<i32, User>>>,
}

#[derive(Deserialize)]
struct CreateUserRequest {
    name: String,
}

async fn read_user(State(state): State<AppState>) -> String {
    let users = state.users.lock().unwrap();
    let mut users = users.values().collect::<Vec<&User>>();
    users.sort_by(|a, b| a.id.cmp(&b.id));

    let json = serde_json::to_string(&users).unwrap();
    println!("response: {}", json);
    json
}

async fn create_user(
    State(state): State<AppState>,
    Json(payload): Json<CreateUserRequest>,
) -> String {
    let mut users = state.users.lock().unwrap();
    let usersvec = users.values().collect::<Vec<&User>>();

    let user = User { 
        id: usersvec.len() as i32 + 1, 
        name: payload.name 
    };

    users.insert(user.id, user.clone());
    
    let json = serde_json::to_string(&user).unwrap();
    println!("response: {}", json);
    json
}

#[tokio::main]
async fn main() {
    let state = AppState {
        users: Arc::new(Mutex::new(HashMap::new())),
    };

    let app = Router::new()
        .route("/users", get(read_user))
        .route("/users/create", post(create_user))
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

    #[tokio::test]
    async fn test_create_user() {
        let state = AppState {
            users: Arc::new(Mutex::new(HashMap::new())),
        };

        let response = create_user(
            State(state.clone()),
            Json(CreateUserRequest { name: "Chad".to_string() })
        ).await;
        assert_eq!(response, "{\"id\":1,\"name\":\"Chad\"}");

        let response = create_user(
            State(state),
            Json(CreateUserRequest { name: "User".to_string() })
        ).await;
        assert_eq!(response, "{\"id\":2,\"name\":\"User\"}");
    }

    #[tokio::test]
    async fn test_read_user() {
        let state = AppState {
            users: Arc::new(Mutex::new(HashMap::new())),
        };

        let response = read_user(State(state.clone())).await;
        assert_eq!(response, "[]");

        let response = create_user(
            State(state.clone()),
            Json(CreateUserRequest { name: "Chad".to_string() })
        ).await;
        assert_eq!(response, "{\"id\":1,\"name\":\"Chad\"}");

        let response = read_user(State(state.clone())).await;
        assert_eq!(response, "[{\"id\":1,\"name\":\"Chad\"}]");

        let response = create_user(
            State(state.clone()),
            Json(CreateUserRequest { name: "User".to_string() })
        ).await;
        assert_eq!(response, "{\"id\":2,\"name\":\"User\"}");

        let response = read_user(State(state)).await;
        assert_eq!(response, "[{\"id\":1,\"name\":\"Chad\"},{\"id\":2,\"name\":\"User\"}]");
    }
}