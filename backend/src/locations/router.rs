use axum::{extract, extract::State, http::StatusCode, middleware, response::IntoResponse, routing, Json, Router};

use crate::common::{
    db::ConnectionPool,
    error::CustomError,
    middleware::{require_admin, require_editor, require_reader, require_writer},
};
use crate::locations::model::UpsertLocation;
use crate::locations::service::LocationsTable;

// - - - - - - - - - - - [ROUTES] - - - - - - - - - - -

pub fn locations_route(shared_connection_pool: ConnectionPool) -> Router {
    let create_routes = Router::new()
        .route("/locations", routing::post(create_location_handler))
        .layer(middleware::from_fn_with_state(
            shared_connection_pool.clone(),
            require_writer,
        ));

    let read_routes = Router::new()
        .route("/locations", routing::get(get_all_locations_handler))
        .route("/locations/:location_id", routing::get(read_location_handler))
        .layer(middleware::from_fn_with_state(
            shared_connection_pool.clone(),
            require_reader,
        ));

    let update_routes = Router::new()
        .route("/locations/:location_id", routing::put(update_location_handler))
        .layer(middleware::from_fn_with_state(
            shared_connection_pool.clone(),
            require_editor,
        ));

    let delete_routes = Router::new()
        .route("/locations/:location_id", routing::delete(delete_location_handler))
        .layer(middleware::from_fn_with_state(
            shared_connection_pool.clone(),
            require_admin,
        ));

    Router::new()
        .merge(create_routes)
        .merge(read_routes)
        .merge(update_routes)
        .merge(delete_routes)
        .with_state(shared_connection_pool)
}

// - - - - - - - - - - - [HANDLERS] - - - - - - - - - - -

fn pool_conn(
    pool: &ConnectionPool,
) -> Result<diesel::r2d2::PooledConnection<diesel::r2d2::ConnectionManager<diesel::PgConnection>>, CustomError> {
    pool.pool
        .get()
        .map_err(|e| CustomError::internal(format!("DB pool: {e}")))
}

pub async fn get_all_locations_handler(
    State(shared_state): State<ConnectionPool>,
) -> Result<impl IntoResponse, CustomError> {
    let locations = LocationsTable::new(pool_conn(&shared_state)?).get_all()?;
    Ok((StatusCode::OK, Json(locations)))
}

pub async fn create_location_handler(
    State(shared_state): State<ConnectionPool>,
    Json(upsert): Json<UpsertLocation>,
) -> Result<impl IntoResponse, CustomError> {
    let created = LocationsTable::new(pool_conn(&shared_state)?).create(upsert)?;
    Ok((StatusCode::CREATED, Json(created)))
}

pub async fn read_location_handler(
    State(shared_state): State<ConnectionPool>,
    extract::Path((location_id,)): extract::Path<(i32,)>,
) -> Result<impl IntoResponse, CustomError> {
    let location = LocationsTable::new(pool_conn(&shared_state)?)
        .get(location_id)?
        .ok_or_else(|| CustomError::not_found("Location not found"))?;
    Ok((StatusCode::OK, Json(location)))
}

pub async fn update_location_handler(
    State(shared_state): State<ConnectionPool>,
    extract::Path((location_id,)): extract::Path<(i32,)>,
    Json(upsert): Json<UpsertLocation>,
) -> Result<impl IntoResponse, CustomError> {
    let updated = LocationsTable::new(pool_conn(&shared_state)?).update(location_id, upsert)?;
    Ok((StatusCode::OK, Json(updated)))
}

pub async fn delete_location_handler(
    State(shared_state): State<ConnectionPool>,
    extract::Path((location_id,)): extract::Path<(i32,)>,
) -> Result<impl IntoResponse, CustomError> {
    LocationsTable::new(pool_conn(&shared_state)?).delete(location_id)?;
    Ok((StatusCode::NO_CONTENT, ()))
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    use crate::common::db::{create_shared_connection_pool, ConnectionPool};
    use crate::common::security::{generate_token, hash_password};
    use crate::common::util::load_environment_variable;
    use crate::locations::model::UpsertLocation;
    use crate::locations::router::locations_route;
    use crate::locations::service::LocationsTable;
    use crate::users::model::{UpsertUser, UserRole};
    use crate::users::service::UsersTable;

    /// Helper: create a user with the given role and return a Bearer token for it.
    pub fn create_user_and_generate_token(
        connection_pool: ConnectionPool,
        email: &str,
        user_role: UserRole,
    ) -> Result<String, jsonwebtoken::errors::Error> {
        let mut new_user = UpsertUser {
            email: email.to_string(),
            role: user_role.as_str().to_string(),
            password: "StålGardinerFunkerFjell53".to_string(),
            fullname: "Josef Stålhard".to_string(),
        };
        hash_password(&mut new_user).expect("Hash failed");

        let create_user_result = {
            let connection = connection_pool.pool.get().expect("Failed to get connection");
            UsersTable::new(connection).create(new_user)
        };

        generate_token(&create_user_result.unwrap())
    }

    #[tokio::test]
    async fn post_locations_returns_201_for_authorized_user_with_write_access() {
        let database_url = load_environment_variable("TEST_DB");
        let connection_pool = create_shared_connection_pool(database_url, 1);
        let service = locations_route(connection_pool.clone());

        let bearer_token =
            create_user_and_generate_token(connection_pool, "stål.hard.russer@ugreit.ru", UserRole::WRITER).unwrap();

        let request_body = UpsertLocation {
            star_system: "Fountain".to_string(),
            area: "The Serpent's Lair".to_string(),
        };

        let request = Request::builder()
            .uri("/locations")
            .method("POST")
            .header("content-type", "application/json")
            .header("Authorization", format!("Bearer {bearer_token}"))
            .body(Body::from(serde_json::to_string(&request_body).unwrap()))
            .unwrap();

        let response = service.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
    }

    #[tokio::test]
    async fn post_locations_returns_401_for_unauthorized_user_without_write_access() {
        let database_url = load_environment_variable("TEST_DB");
        let connection_pool = create_shared_connection_pool(database_url, 1);
        let service = locations_route(connection_pool.clone());

        let bearer_token =
            create_user_and_generate_token(connection_pool, "myk.og.ekkel.russer@put.in", UserRole::READER).unwrap();

        let request_body = UpsertLocation {
            star_system: "Fountain".to_string(),
            area: "The Serpent's Lair".to_string(),
        };

        let request = Request::builder()
            .uri("/locations")
            .method("POST")
            .header("content-type", "application/json")
            .header("Authorization", format!("Bearer {bearer_token}"))
            .body(Body::from(serde_json::to_string(&request_body).unwrap()))
            .unwrap();

        let response = service.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn get_locations_returns_401_when_missing_authorization_header() {
        let database_url = load_environment_variable("TEST_DB");
        let connection_pool = create_shared_connection_pool(database_url, 1);
        let service = locations_route(connection_pool);

        let request = Request::builder()
            .uri("/locations")
            .method("GET")
            .body(Body::empty())
            .unwrap();

        let response = service.oneshot(request).await.unwrap();
        // Used to be 500 — now correctly 401.
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn get_locations_returns_404_on_non_existing_id() {
        let database_url = load_environment_variable("TEST_DB");
        let connection_pool = create_shared_connection_pool(database_url, 2);
        let service = locations_route(connection_pool.clone());

        let bearer_token =
            create_user_and_generate_token(connection_pool, "birdman@ifi.uio.no", UserRole::READER).unwrap();

        let request = Request::builder()
            .uri(format!("/locations/{}", -666))
            .method("GET")
            .header("Authorization", format!("Bearer {bearer_token}"))
            .body(Body::empty())
            .unwrap();

        let response = service.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn delete_locations_returns_204_for_authorized_user_with_admin_role() {
        let database_url = load_environment_variable("TEST_DB");
        let connection_pool = create_shared_connection_pool(database_url, 2);
        let connection = connection_pool.pool.get().expect("Failed to get connection");
        let mut location_db = LocationsTable::new(connection);
        let service = locations_route(connection_pool.clone());

        let bearer_token = create_user_and_generate_token(
            connection_pool,
            "you.know.your.judo.well@succulentmail.gb",
            UserRole::ADMIN,
        )
        .unwrap();

        let created = location_db
            .create(UpsertLocation {
                star_system: "Fountain".to_string(),
                area: "The Serpent's Lair".to_string(),
            })
            .expect("Create location failed");

        let request = Request::builder()
            .uri(format!("/locations/{}", created.id))
            .method("DELETE")
            .header("Authorization", format!("Bearer {bearer_token}"))
            .body(Body::empty())
            .unwrap();

        let response = service.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);

        let after = location_db.get(created.id).expect("read after delete");
        assert!(after.is_none());
    }
}
