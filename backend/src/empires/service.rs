use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, PooledConnection};
use diesel::PgConnection;

use crate::common::error::CustomError;
use crate::empires::model::{Empire, UpsertEmpire};
use crate::schema;

type PooledPg = PooledConnection<ConnectionManager<PgConnection>>;

pub struct EmpiresTable {
    connection: PooledPg,
}

impl EmpiresTable {
    pub fn new(connection: PooledPg) -> Self {
        Self { connection }
    }

    pub fn create(&mut self, upsert: UpsertEmpire) -> Result<Empire, CustomError> {
        use schema::empires;

        diesel::insert_into(empires::table)
            .values((
                empires::name.eq(&upsert.name),
                empires::slogan.eq(&upsert.slogan),
                empires::location_id.eq(&upsert.location_id),
                empires::description.eq(&upsert.description),
            ))
            .get_result(&mut self.connection)
            .map_err(|err| CustomError::from_diesel_err(err, "while creating empire"))
    }

    pub fn get_all(&mut self) -> Result<Vec<Empire>, CustomError> {
        use schema::empires;
        empires::table
            .load::<Empire>(&mut self.connection)
            .map_err(|err| CustomError::from_diesel_err(err, "while listing empires"))
    }

    pub fn get(&mut self, empire_id: i32) -> Result<Option<Empire>, CustomError> {
        use schema::empires;
        empires::table
            .find(empire_id)
            .get_result(&mut self.connection)
            .optional()
            .map_err(|err| CustomError::from_diesel_err(err, "while reading empire"))
    }

    pub fn update(&mut self, empire_id: i32, upsert: UpsertEmpire) -> Result<Empire, CustomError> {
        use schema::empires;

        let updated = diesel::update(empires::table.find(empire_id))
            .set((
                empires::name.eq(&upsert.name),
                empires::slogan.eq(&upsert.slogan),
                empires::location_id.eq(upsert.location_id),
                empires::description.eq(&upsert.description),
            ))
            .get_result::<Empire>(&mut self.connection);

        match updated {
            Ok(e) => Ok(e),
            Err(diesel::result::Error::NotFound) => Err(CustomError::not_found("Empire not found")),
            Err(err) => Err(CustomError::from_diesel_err(err, "while updating empire")),
        }
    }

    pub fn delete(&mut self, empire_id: i32) -> Result<(), CustomError> {
        use schema::empires;
        let rows = diesel::delete(empires::table.find(empire_id))
            .execute(&mut self.connection)
            .map_err(|err| CustomError::from_diesel_err(err, "while deleting empire"))?;
        if rows == 0 {
            Err(CustomError::not_found("Empire not found"))
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
    use crate::empires::model::UpsertEmpire;
    use crate::empires::service::EmpiresTable;
    use crate::locations::model::UpsertLocation;
    use crate::locations::service::LocationsTable;

    fn pool() -> crate::common::db::ConnectionPool {
        let database_url = load_environment_variable("TEST_DB");
        create_shared_connection_pool(database_url, 2)
    }

    fn make_location(p: &crate::common::db::ConnectionPool) -> i32 {
        let conn = p.pool.get().expect("conn");
        LocationsTable::new(conn)
            .create(UpsertLocation {
                star_system: "Stub System".to_string(),
                area: "Stub Area".to_string(),
            })
            .expect("create location")
            .id
    }

    #[test]
    fn create_and_read_succeed() {
        let p = pool();
        let location_id = make_location(&p);
        let conn = p.pool.get().expect("conn");
        let mut db = EmpiresTable::new(conn);

        let new_empire = UpsertEmpire {
            name: "Test Empire".to_string(),
            slogan: "for science".to_string(),
            location_id,
            description: "An empire created in a test.".to_string(),
        };
        let created = db.create(new_empire.clone()).expect("create empire");
        assert_eq!(created.name, new_empire.name);

        let read = db.get(created.id).expect("get").unwrap();
        assert_eq!(read.id, created.id);
    }

    #[test]
    fn update_fails_on_nonexistent_id() {
        let p = pool();
        let location_id = make_location(&p);
        let conn = p.pool.get().expect("conn");
        let mut db = EmpiresTable::new(conn);

        let upsert = UpsertEmpire {
            name: "missing".to_string(),
            slogan: "x".to_string(),
            location_id,
            description: "y".to_string(),
        };
        let result = db.update(-666, upsert);
        assert!(matches!(result, Err(e) if e.err_type == ErrorType::NotFound));
    }

    #[test]
    fn delete_fails_on_nonexistent_id() {
        let p = pool();
        let conn = p.pool.get().expect("conn");
        let mut db = EmpiresTable::new(conn);

        let result = db.delete(-666);
        assert!(matches!(result, Err(e) if e.err_type == ErrorType::NotFound));
    }
}
