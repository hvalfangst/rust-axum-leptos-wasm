use axum::{extract, extract::State, http::StatusCode, middleware, response::IntoResponse, routing, Json, Router};

use crate::common::{
    db::ConnectionPool,
    error::CustomError,
    middleware::{require_admin, require_editor, require_reader, require_writer},
};
use crate::empires::model::UpsertEmpire;
use crate::empires::service::EmpiresTable;

pub fn empires_route(shared_connection_pool: ConnectionPool) -> Router {
    let create_routes = Router::new()
        .route("/empires", routing::post(create_empire_handler))
        .layer(middleware::from_fn_with_state(
            shared_connection_pool.clone(),
            require_writer,
        ));

    let read_routes = Router::new()
        .route("/empires", routing::get(get_all_empires_handler))
        .route("/empires/:empire_id", routing::get(read_empire_handler))
        .layer(middleware::from_fn_with_state(
            shared_connection_pool.clone(),
            require_reader,
        ));

    let update_routes = Router::new()
        .route("/empires/:empire_id", routing::put(update_empire_handler))
        .layer(middleware::from_fn_with_state(
            shared_connection_pool.clone(),
            require_editor,
        ));

    let delete_routes = Router::new()
        .route("/empires/:empire_id", routing::delete(delete_empire_handler))
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

fn pool_conn(
    pool: &ConnectionPool,
) -> Result<diesel::r2d2::PooledConnection<diesel::r2d2::ConnectionManager<diesel::PgConnection>>, CustomError> {
    pool.pool
        .get()
        .map_err(|e| CustomError::internal(format!("DB pool: {e}")))
}

pub async fn get_all_empires_handler(
    State(shared_state): State<ConnectionPool>,
) -> Result<impl IntoResponse, CustomError> {
    let empires = EmpiresTable::new(pool_conn(&shared_state)?).get_all()?;
    Ok((StatusCode::OK, Json(empires)))
}

pub async fn create_empire_handler(
    State(shared_state): State<ConnectionPool>,
    Json(upsert): Json<UpsertEmpire>,
) -> Result<impl IntoResponse, CustomError> {
    let created = EmpiresTable::new(pool_conn(&shared_state)?).create(upsert)?;
    Ok((StatusCode::CREATED, Json(created)))
}

pub async fn read_empire_handler(
    State(shared_state): State<ConnectionPool>,
    extract::Path((empire_id,)): extract::Path<(i32,)>,
) -> Result<impl IntoResponse, CustomError> {
    let empire = EmpiresTable::new(pool_conn(&shared_state)?)
        .get(empire_id)?
        .ok_or_else(|| CustomError::not_found("Empire not found"))?;
    Ok((StatusCode::OK, Json(empire)))
}

pub async fn update_empire_handler(
    State(shared_state): State<ConnectionPool>,
    extract::Path((empire_id,)): extract::Path<(i32,)>,
    Json(upsert): Json<UpsertEmpire>,
) -> Result<impl IntoResponse, CustomError> {
    let updated = EmpiresTable::new(pool_conn(&shared_state)?).update(empire_id, upsert)?;
    Ok((StatusCode::OK, Json(updated)))
}

pub async fn delete_empire_handler(
    State(shared_state): State<ConnectionPool>,
    extract::Path((empire_id,)): extract::Path<(i32,)>,
) -> Result<impl IntoResponse, CustomError> {
    EmpiresTable::new(pool_conn(&shared_state)?).delete(empire_id)?;
    Ok((StatusCode::NO_CONTENT, ()))
}
