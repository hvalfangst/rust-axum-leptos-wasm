use axum::{
    body::Body,
    extract::State,
    http::Request,
    middleware::Next,
    response::{IntoResponse, Response},
};

use crate::common::{db::ConnectionPool, security::authorize_with_role};
use crate::users::model::{User, UserRole};

/// Extension storing the authorized user on the request.
/// The field is read by handlers via `req.extensions().get::<AuthorizedUser>()`.
#[allow(dead_code)]
pub struct AuthorizedUser(pub User);

pub async fn require_admin(State(pool): State<ConnectionPool>, req: Request<Body>, next: Next<Body>) -> Response {
    authorize_and_continue(req, next, pool, UserRole::ADMIN).await
}

pub async fn require_editor(State(pool): State<ConnectionPool>, req: Request<Body>, next: Next<Body>) -> Response {
    authorize_and_continue(req, next, pool, UserRole::EDITOR).await
}

pub async fn require_writer(State(pool): State<ConnectionPool>, req: Request<Body>, next: Next<Body>) -> Response {
    authorize_and_continue(req, next, pool, UserRole::WRITER).await
}

pub async fn require_reader(State(pool): State<ConnectionPool>, req: Request<Body>, next: Next<Body>) -> Response {
    authorize_and_continue(req, next, pool, UserRole::READER).await
}

async fn authorize_and_continue(
    mut req: Request<Body>,
    next: Next<Body>,
    pool: ConnectionPool,
    required_role: UserRole,
) -> Response {
    match authorize_with_role(req.headers(), &pool, required_role).await {
        Ok(user) => {
            req.extensions_mut().insert(AuthorizedUser(user));
            next.run(req).await
        }
        Err(err) => err.into_response(),
    }
}
