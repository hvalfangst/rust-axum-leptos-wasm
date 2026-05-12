use axum::{extract, extract::State, http::StatusCode, middleware, response::IntoResponse, routing, Json, Router};
use bcrypt::verify;
use serde_json::{json, Value};
use tracing::warn;

use crate::common::{
    db::ConnectionPool,
    error::CustomError,
    middleware::{require_admin, require_editor, require_reader},
    security::{generate_token, hash_password},
};
use crate::users::model::{is_valid_email, LoginUser, RegisterUser, UpdateUser, UpsertUser, User, UserRole};
use crate::users::service::UsersTable;

// - - - - - - - - - - - [ROUTES] - - - - - - - - - - -

pub fn users_route(shared_connection_pool: ConnectionPool) -> Router {
    let public_routes = Router::new()
        .route("/users", routing::post(create_user_handler))
        .route("/users/login", routing::post(login_user_handler));

    let read_routes = Router::new()
        .route("/users", routing::get(list_users_handler))
        .route("/users/:user_id", routing::get(get_user_handler))
        .layer(middleware::from_fn_with_state(
            shared_connection_pool.clone(),
            require_reader,
        ));

    let update_routes = Router::new()
        .route("/users/:user_id", routing::put(update_user_handler))
        .layer(middleware::from_fn_with_state(
            shared_connection_pool.clone(),
            require_editor,
        ));

    let delete_routes = Router::new()
        .route("/users/:user_id", routing::delete(delete_user_handler))
        .layer(middleware::from_fn_with_state(
            shared_connection_pool.clone(),
            require_admin,
        ));

    Router::new()
        .merge(public_routes)
        .merge(read_routes)
        .merge(update_routes)
        .merge(delete_routes)
        .with_state(shared_connection_pool)
}

// - - - - - - - - - - - [HANDLERS] - - - - - - - - - - -

pub async fn list_users_handler(State(shared_state): State<ConnectionPool>) -> Result<impl IntoResponse, CustomError> {
    let connection = shared_state
        .pool
        .get()
        .map_err(|e| CustomError::internal(format!("DB pool: {e}")))?;
    let users = UsersTable::new(connection).list()?;
    Ok((StatusCode::OK, Json(users)))
}

/// Public registration. Role is server-assigned (always READER) — a client
/// cannot escalate privileges by sending `role: "ADMIN"`.
pub async fn create_user_handler(
    State(shared_state): State<ConnectionPool>,
    Json(body): Json<RegisterUser>,
) -> Result<impl IntoResponse, CustomError> {
    if !is_valid_email(&body.email) {
        return Err(CustomError::validation("Invalid input for field 'email'"));
    }

    let mut upsert = UpsertUser {
        email: body.email,
        password: body.password,
        fullname: body.fullname,
        role: UserRole::READER.as_str().to_string(),
    };
    hash_password(&mut upsert)?;

    let connection = shared_state
        .pool
        .get()
        .map_err(|e| CustomError::internal(format!("DB pool: {e}")))?;

    let created = UsersTable::new(connection).create(upsert)?;
    Ok((StatusCode::CREATED, Json(created)))
}

pub async fn get_user_handler(
    State(shared_state): State<ConnectionPool>,
    extract::Path((user_id,)): extract::Path<(i32,)>,
) -> Result<impl IntoResponse, CustomError> {
    let connection = shared_state
        .pool
        .get()
        .map_err(|e| CustomError::internal(format!("DB pool: {e}")))?;

    let user = UsersTable::new(connection)
        .get(user_id)?
        .ok_or_else(|| CustomError::not_found("User not found"))?;
    Ok((StatusCode::OK, Json(user)))
}

pub async fn update_user_handler(
    State(shared_state): State<ConnectionPool>,
    extract::Path((user_id,)): extract::Path<(i32,)>,
    Json(body): Json<UpdateUser>,
) -> Result<impl IntoResponse, CustomError> {
    if !is_valid_email(&body.email) {
        return Err(CustomError::validation("Invalid input for field 'email'"));
    }
    let role =
        UserRole::try_from_str(&body.role).ok_or_else(|| CustomError::validation("Invalid input for field 'role'"))?;

    let connection = shared_state
        .pool
        .get()
        .map_err(|e| CustomError::internal(format!("DB pool: {e}")))?;
    let mut users = UsersTable::new(connection);

    // If a new password was supplied, hash it; otherwise reuse the existing one.
    let existing = users
        .get(user_id)?
        .ok_or_else(|| CustomError::not_found("User not found"))?;
    let password = match body.password {
        Some(pw) if !pw.is_empty() => {
            let mut tmp = UpsertUser {
                email: existing.email.clone(),
                password: pw,
                fullname: existing.fullname.clone(),
                role: existing.role.clone(),
            };
            hash_password(&mut tmp)?;
            tmp.password
        }
        _ => existing.password,
    };

    let upsert = UpsertUser {
        email: body.email,
        password,
        fullname: body.fullname,
        role: role.as_str().to_string(),
    };
    let updated = users.update(user_id, upsert)?;
    Ok((StatusCode::OK, Json(updated)))
}

pub async fn delete_user_handler(
    State(shared_state): State<ConnectionPool>,
    extract::Path((user_id,)): extract::Path<(i32,)>,
) -> Result<impl IntoResponse, CustomError> {
    let connection = shared_state
        .pool
        .get()
        .map_err(|e| CustomError::internal(format!("DB pool: {e}")))?;
    UsersTable::new(connection).delete(user_id)?;
    Ok((StatusCode::NO_CONTENT, ()))
}

pub async fn login_user_handler(
    State(shared_state): State<ConnectionPool>,
    Json(body): Json<LoginUser>,
) -> Result<impl IntoResponse, (StatusCode, Json<Value>)> {
    let connection = shared_state.pool.get().map_err(|e| {
        warn!(error = %e, "DB pool acquisition failed");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Internal server error"})),
        )
    })?;

    let user_lookup = UsersTable::new(connection).get_by_email(&body.email);
    match user_lookup {
        Ok(Some(user)) => {
            if verify(&body.password, &user.password).unwrap_or(false) {
                let token = generate_token(&user).map_err(|_| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({"error": "Failed to generate token"})),
                    )
                })?;
                Ok((StatusCode::OK, Json(token)))
            } else {
                // Same response for "no such user" and "wrong password" — don't
                // help attackers enumerate registered emails.
                Err((StatusCode::UNAUTHORIZED, Json(json!({"error": "Invalid credentials"}))))
            }
        }
        Ok(None) => Err((StatusCode::UNAUTHORIZED, Json(json!({"error": "Invalid credentials"})))),
        Err(err) => {
            warn!(error = %err, "login failed");
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Internal server error"})),
            ))
        }
    }
}

// Silence dead_code lint: User is constructed via Diesel queries.
#[allow(dead_code)]
fn _assert_user_serializes(_u: &User) {}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use serde_json::json;
    use tower::ServiceExt;

    use crate::common::db::create_shared_connection_pool;
    use crate::common::util::load_environment_variable;
    use crate::users::router::users_route;

    #[tokio::test]
    async fn post_users_returns_201_on_valid_data() {
        let database_url = load_environment_variable("TEST_DB");
        let connection_pool = create_shared_connection_pool(database_url, 1);
        let service = users_route(connection_pool);

        let body = json!({
            "email": "valid@email.com",
            "password": "Big100",
            "fullname": "Kenneth Molasses"
        });

        let request = Request::builder()
            .uri("/users")
            .method("POST")
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap();

        let response = service.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
    }

    #[tokio::test]
    async fn post_users_returns_422_on_invalid_email() {
        let database_url = load_environment_variable("TEST_DB");
        let connection_pool = create_shared_connection_pool(database_url, 1);
        let service = users_route(connection_pool);

        let body = json!({
            "email": "eg-klare-meg",
            "password": "Big100",
            "fullname": "Kenneth Molasses"
        });

        let request = Request::builder()
            .uri("/users")
            .method("POST")
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap();

        let response = service.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn post_users_ignores_role_in_body() {
        let database_url = load_environment_variable("TEST_DB");
        let connection_pool = create_shared_connection_pool(database_url, 1);
        let service = users_route(connection_pool.clone());

        // Even if the client sends role=ADMIN, registration must produce a READER.
        let body = json!({
            "email": "sneaky-admin@example.com",
            "password": "letmein",
            "fullname": "Sneaky",
            "role": "ADMIN"
        });

        let request = Request::builder()
            .uri("/users")
            .method("POST")
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap();

        let response = service.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);

        let body_bytes = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let user: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
        assert_eq!(user["role"], "READER");
        // Password hash must never be serialized back to the client.
        assert!(user.get("password").is_none());
    }
}
