use std::collections::HashMap;
use std::time::{Duration, SystemTime};

use bcrypt::hash;
use http::HeaderMap;
use jsonwebtoken::{
    decode, encode, errors::ErrorKind as JwtErrorKind, Algorithm, DecodingKey, EncodingKey, Header, TokenData,
    Validation,
};
use once_cell::sync::Lazy;
use tracing::debug;

use crate::common::{config::Config, db::ConnectionPool, error::CustomError};
use crate::users::model::{Claims, UpsertUser, User, UserRole};
use crate::users::service::UsersTable as UsersDB;

const BCRYPT_COST: u32 = 12;
const TOKEN_TTL_SECS: u64 = 3600;

/// Role inclusion map. Built once.
static ROLE_HIERARCHY: Lazy<HashMap<UserRole, &'static [UserRole]>> = Lazy::new(|| {
    let mut m = HashMap::new();
    m.insert(
        UserRole::ADMIN,
        &[UserRole::ADMIN, UserRole::EDITOR, UserRole::WRITER, UserRole::READER][..],
    );
    m.insert(
        UserRole::EDITOR,
        &[UserRole::EDITOR, UserRole::WRITER, UserRole::READER][..],
    );
    m.insert(UserRole::WRITER, &[UserRole::WRITER, UserRole::READER][..]);
    m.insert(UserRole::READER, &[UserRole::READER][..]);
    m
});

pub fn hash_password(body: &mut UpsertUser) -> Result<(), CustomError> {
    body.password = hash(&body.password, BCRYPT_COST).map_err(|_| CustomError::internal("Failed to hash password"))?;
    Ok(())
}

pub fn generate_token(user: &User) -> Result<String, jsonwebtoken::errors::Error> {
    let expiration = SystemTime::now()
        .checked_add(Duration::from_secs(TOKEN_TTL_SECS))
        .expect("Failed to calculate token expiration")
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("SystemTime before UNIX EPOCH")
        .as_secs() as i64;

    let claims = Claims {
        sub: user.email.clone(),
        role: UserRole::from_str(&user.role),
        exp: expiration,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(Config::get().encryption_key.as_ref()),
    )
}

pub fn decode_claims(headers: &HeaderMap) -> Result<TokenData<Claims>, CustomError> {
    // Retrieve Authorization header
    let token_header = headers
        .get("Authorization")
        .ok_or_else(|| CustomError::unauthorized("Missing Authorization header"))?;

    let token = token_header
        .to_str()
        .map_err(|_| CustomError::unauthorized("Authorization header is not valid UTF-8"))?;

    let raw = token
        .strip_prefix("Bearer ")
        .ok_or_else(|| CustomError::unauthorized("Token is missing 'Bearer ' prefix"))?;

    match decode::<Claims>(
        raw,
        &DecodingKey::from_secret(Config::get().encryption_key.as_ref()),
        &Validation::new(Algorithm::HS256),
    ) {
        Ok(decoded) => Ok(decoded),
        Err(err) => match err.kind() {
            JwtErrorKind::ExpiredSignature => Err(CustomError::unauthorized("Token has expired")),
            _ => {
                debug!(?err, "JWT decode failed");
                Err(CustomError::unauthorized("Invalid JWT"))
            }
        },
    }
}

pub async fn authorize_with_role(
    headers: &HeaderMap,
    shared_state: &ConnectionPool,
    required_role: UserRole,
) -> Result<User, CustomError> {
    let claims = decode_claims(headers)?;
    enforce_role_policy(shared_state, &claims, required_role).await
}

pub async fn enforce_role_policy(
    shared_state: &ConnectionPool,
    claims: &TokenData<Claims>,
    required_role: UserRole,
) -> Result<User, CustomError> {
    let connection = shared_state
        .pool
        .get()
        .map_err(|e| CustomError::internal(format!("DB pool: {e}")))?;
    let mut users = UsersDB::new(connection);

    let user = users
        .get_by_email(&claims.claims.sub)?
        .ok_or_else(|| CustomError::unauthorized("User in claims not found"))?;

    let user_role = UserRole::from_str(&user.role);
    let allowed = ROLE_HIERARCHY
        .get(&user_role)
        .map(|roles| roles.contains(&required_role))
        .unwrap_or(false);

    if allowed {
        debug!(role = %user_role, required = %required_role, "access granted");
        Ok(user)
    } else {
        Err(CustomError::unauthorized(format!(
            "Current role of {user_role} does not have access to {required_role}"
        )))
    }
}
