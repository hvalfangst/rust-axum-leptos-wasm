use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, PooledConnection};
use diesel::PgConnection;

use crate::common::error::CustomError;
use crate::schema;
use crate::users::model::{UpsertUser, User};

type PooledPg = PooledConnection<ConnectionManager<PgConnection>>;

pub struct UsersTable {
    connection: PooledPg,
}

impl UsersTable {
    pub fn new(connection: PooledPg) -> Self {
        Self { connection }
    }

    pub fn create(&mut self, new_user: UpsertUser) -> Result<User, CustomError> {
        use schema::users;

        diesel::insert_into(users::table)
            .values((
                users::email.eq(&new_user.email),
                users::password.eq(&new_user.password),
                users::fullname.eq(&new_user.fullname),
                users::role.eq(&new_user.role),
            ))
            .get_result::<User>(&mut self.connection)
            .map_err(|err| CustomError::from_diesel_err(err, "while creating user"))
    }

    pub fn get(&mut self, user_id: i32) -> Result<Option<User>, CustomError> {
        use schema::users;

        users::table
            .find(user_id)
            .get_result(&mut self.connection)
            .optional()
            .map_err(|err| CustomError::from_diesel_err(err, "while reading user"))
    }

    pub fn get_by_email(&mut self, email: &str) -> Result<Option<User>, CustomError> {
        use schema::users;

        users::table
            .filter(users::email.eq(email))
            .get_result(&mut self.connection)
            .optional()
            .map_err(|err| CustomError::from_diesel_err(err, "while reading user by email"))
    }

    pub fn list(&mut self) -> Result<Vec<User>, CustomError> {
        use schema::users;

        users::table
            .load::<User>(&mut self.connection)
            .map_err(|err| CustomError::from_diesel_err(err, "while listing users"))
    }

    pub fn update(&mut self, user_id: i32, update: UpsertUser) -> Result<User, CustomError> {
        use schema::users;

        let updated = diesel::update(users::table.find(user_id))
            .set((
                users::email.eq(&update.email),
                users::password.eq(&update.password),
                users::fullname.eq(&update.fullname),
                users::role.eq(&update.role),
            ))
            .get_result::<User>(&mut self.connection);

        match updated {
            Ok(user) => Ok(user),
            Err(diesel::result::Error::NotFound) => Err(CustomError::not_found("User not found")),
            Err(err) => Err(CustomError::from_diesel_err(err, "while updating user")),
        }
    }

    pub fn delete(&mut self, user_id: i32) -> Result<(), CustomError> {
        use schema::users;

        let rows = diesel::delete(users::table.find(user_id))
            .execute(&mut self.connection)
            .map_err(|err| CustomError::from_diesel_err(err, "while deleting user"))?;

        if rows == 0 {
            Err(CustomError::not_found("User not found"))
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::common::db::create_shared_connection_pool;
    use crate::common::error::ErrorType;
    use crate::common::util::load_environment_variable;
    use crate::users::model::UpsertUser;
    use crate::users::service::UsersTable;

    fn pool() -> crate::common::db::ConnectionPool {
        let database_url = load_environment_variable("TEST_DB");
        create_shared_connection_pool(database_url, 2)
    }

    #[test]
    fn create_succeeds_on_valid_input() {
        let connection_pool = pool();
        let connection = connection_pool.pool.get().expect("Failed to get connection");
        let mut user_db = UsersTable::new(connection);

        let new_user = UpsertUser {
            email: "obelisksx@ifi.uio.no".to_string(),
            password: "EatSleepRepeat".to_string(),
            fullname: "Obelix fra IFI".to_string(),
            role: "READER".to_string(),
        };

        let created = user_db.create(new_user.clone()).expect("Create user failed");
        assert_eq!(created.email, new_user.email);
        assert_eq!(created.password, new_user.password);
        assert_eq!(created.fullname, new_user.fullname);
        assert_eq!(created.role, new_user.role);
    }

    #[test]
    fn create_fails_on_duplicate_mail() {
        let connection_pool = pool();
        let connection = connection_pool.pool.get().expect("Failed to get connection");
        let mut user_db = UsersTable::new(connection);

        let dupe_user = UpsertUser {
            email: "duperdave@blizzard.com".to_string(),
            password: "GullDagger69".to_string(),
            fullname: "Mule Duperino".to_string(),
            role: "READER".to_string(),
        };

        user_db.create(dupe_user.clone()).expect("First create failed");
        let second_create = user_db.create(dupe_user.clone());
        let err = second_create.expect_err("Expected duplicate-mail error");
        assert_eq!(err.err_type, ErrorType::UniqueViolation);
    }

    #[test]
    fn read_returns_none_on_non_existing_id() {
        let connection_pool = pool();
        let connection = connection_pool.pool.get().expect("Failed to get connection");
        let mut user_db = UsersTable::new(connection);

        let retrieved = user_db.get(-666).expect("get should not error on missing id");
        assert!(retrieved.is_none());
    }

    #[test]
    fn update_fails_on_nonexistent_id() {
        let connection_pool = pool();
        let connection = connection_pool.pool.get().expect("Failed to get connection");
        let mut user_db = UsersTable::new(connection);

        let request = UpsertUser {
            email: "lukewarm@manlet.com".to_string(),
            password: "realfrogeyes".to_string(),
            fullname: "Lukas Parrot".to_string(),
            role: "READER".to_string(),
        };

        let result = user_db.update(-666, request);
        assert!(matches!(result, Err(e) if e.err_type == ErrorType::NotFound));
    }

    #[test]
    fn delete_fails_on_nonexistent_id() {
        let connection_pool = pool();
        let connection = connection_pool.pool.get().expect("Failed to get connection");
        let mut user_db = UsersTable::new(connection);

        let result = user_db.delete(-666);
        assert!(matches!(result, Err(e) if e.err_type == ErrorType::NotFound));
    }

    #[test]
    fn list_returns_users() {
        let connection_pool = pool();
        let connection = connection_pool.pool.get().expect("Failed to get connection");
        let mut user_db = UsersTable::new(connection);

        let users = user_db.list().expect("List users failed");
        // Just smoke-check; ordering and counts depend on other tests.
        let _ = users.len();
    }
}
