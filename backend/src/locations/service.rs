use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, PooledConnection};
use diesel::PgConnection;

use crate::common::error::CustomError;
use crate::locations::model::{Location, UpsertLocation};
use crate::schema;

type PooledPg = PooledConnection<ConnectionManager<PgConnection>>;

pub struct LocationsTable {
    connection: PooledPg,
}

impl LocationsTable {
    pub fn new(connection: PooledPg) -> Self {
        Self { connection }
    }

    pub fn create(&mut self, upsert: UpsertLocation) -> Result<Location, CustomError> {
        use schema::locations;

        diesel::insert_into(locations::table)
            .values((
                locations::star_system.eq(&upsert.star_system),
                locations::area.eq(&upsert.area),
            ))
            .get_result(&mut self.connection)
            .map_err(|err| CustomError::from_diesel_err(err, "while creating location"))
    }

    pub fn get_all(&mut self) -> Result<Vec<Location>, CustomError> {
        use schema::locations;
        locations::table
            .load::<Location>(&mut self.connection)
            .map_err(|err| CustomError::from_diesel_err(err, "while listing locations"))
    }

    pub fn get(&mut self, location_id: i32) -> Result<Option<Location>, CustomError> {
        use schema::locations;
        locations::table
            .find(location_id)
            .get_result(&mut self.connection)
            .optional()
            .map_err(|err| CustomError::from_diesel_err(err, "while reading location"))
    }

    pub fn update(&mut self, location_id: i32, upsert: UpsertLocation) -> Result<Location, CustomError> {
        use schema::locations;

        let updated = diesel::update(locations::table.find(location_id))
            .set((
                locations::star_system.eq(&upsert.star_system),
                locations::area.eq(&upsert.area),
            ))
            .get_result::<Location>(&mut self.connection);

        match updated {
            Ok(loc) => Ok(loc),
            Err(diesel::result::Error::NotFound) => Err(CustomError::not_found("Location not found")),
            Err(err) => Err(CustomError::from_diesel_err(err, "while updating location")),
        }
    }

    pub fn delete(&mut self, location_id: i32) -> Result<(), CustomError> {
        use schema::locations;
        let rows = diesel::delete(locations::table.find(location_id))
            .execute(&mut self.connection)
            .map_err(|err| CustomError::from_diesel_err(err, "while deleting location"))?;
        if rows == 0 {
            Err(CustomError::not_found("Location not found"))
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
    use crate::locations::model::UpsertLocation;
    use crate::locations::service::LocationsTable;

    fn pool() -> crate::common::db::ConnectionPool {
        let database_url = load_environment_variable("TEST_DB");
        create_shared_connection_pool(database_url, 2)
    }

    #[test]
    fn create_succeeds_on_valid_input() {
        let p = pool();
        let connection = p.pool.get().expect("Failed to get connection");
        let mut db = LocationsTable::new(connection);

        let new_loc = UpsertLocation {
            star_system: "Test Star System".to_string(),
            area: "Test Area".to_string(),
        };
        let created = db.create(new_loc.clone()).expect("Create location failed");
        assert_eq!(created.star_system, new_loc.star_system);
        assert_eq!(created.area, new_loc.area);
    }

    #[test]
    fn read_returns_none_on_nonexistent_id() {
        let p = pool();
        let connection = p.pool.get().expect("Failed to get connection");
        let mut db = LocationsTable::new(connection);

        let result = db.get(-666).expect("get should not error on missing id");
        assert!(result.is_none());
    }

    #[test]
    fn update_fails_on_nonexistent_id() {
        let p = pool();
        let connection = p.pool.get().expect("Failed to get connection");
        let mut db = LocationsTable::new(connection);

        let request = UpsertLocation {
            star_system: "x".to_string(),
            area: "y".to_string(),
        };
        let result = db.update(-1, request);
        assert!(matches!(result, Err(e) if e.err_type == ErrorType::NotFound));
    }

    #[test]
    fn delete_fails_on_nonexistent_id() {
        let p = pool();
        let connection = p.pool.get().expect("Failed to get connection");
        let mut db = LocationsTable::new(connection);

        let result = db.delete(-666);
        assert!(matches!(result, Err(e) if e.err_type == ErrorType::NotFound));
    }
}
