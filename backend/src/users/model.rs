use std::fmt;

use diesel::prelude::*;
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::schema::users;

#[derive(Debug, Clone, Serialize, Queryable)]
#[diesel(table_name = users)]
pub struct User {
    pub id: i32,
    pub email: String,
    #[serde(skip_serializing)]
    pub password: String,
    pub fullname: String,
    pub role: String,
}

// Variant names match the on-disk string representation in `users.role`, so
// we deliberately keep them ALL_CAPS rather than the idiomatic camel case.
#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum UserRole {
    READER,
    WRITER,
    EDITOR,
    ADMIN,
}

impl UserRole {
    pub fn as_str(self) -> &'static str {
        match self {
            UserRole::READER => "READER",
            UserRole::WRITER => "WRITER",
            UserRole::EDITOR => "EDITOR",
            UserRole::ADMIN => "ADMIN",
        }
    }

    /// Parse a role string; unknown values default to READER (least privilege).
    pub fn from_str(value: &str) -> UserRole {
        match value {
            "WRITER" => UserRole::WRITER,
            "EDITOR" => UserRole::EDITOR,
            "ADMIN" => UserRole::ADMIN,
            _ => UserRole::READER,
        }
    }

    /// Strict parse used at API boundaries: returns None for unknown values.
    pub fn try_from_str(value: &str) -> Option<UserRole> {
        match value {
            "READER" => Some(UserRole::READER),
            "WRITER" => Some(UserRole::WRITER),
            "EDITOR" => Some(UserRole::EDITOR),
            "ADMIN" => Some(UserRole::ADMIN),
            _ => None,
        }
    }
}

impl fmt::Display for UserRole {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Used internally for inserts/updates. `role` is set by the server, never the
/// raw client body — see `RegisterUser`/`UpdateUser` for the request DTOs.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = users)]
pub struct UpsertUser {
    pub email: String,
    pub password: String,
    pub fullname: String,
    pub role: String,
}

/// Public registration payload. Note: no `role` — new users are always READER.
#[derive(Debug, Clone, Deserialize)]
pub struct RegisterUser {
    pub email: String,
    pub password: String,
    pub fullname: String,
}

/// Admin/editor update payload. Password is optional (omit to keep current).
#[derive(Debug, Clone, Deserialize)]
pub struct UpdateUser {
    pub email: String,
    pub password: Option<String>,
    pub fullname: String,
    pub role: String,
}

static EMAIL_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"^[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}$").unwrap());

pub fn is_valid_email(email: &str) -> bool {
    EMAIL_RE.is_match(email)
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoginUser {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub exp: i64,
    pub role: UserRole,
}
