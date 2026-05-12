use diesel::r2d2::{ConnectionManager, Pool};
use diesel::PgConnection;

#[derive(Clone)]
pub struct ConnectionPool {
    pub pool: Pool<ConnectionManager<PgConnection>>,
}

pub fn create_shared_connection_pool(database_url: String, max_size: u32) -> ConnectionPool {
    let manager = ConnectionManager::<PgConnection>::new(database_url);

    let pool = Pool::builder()
        .max_size(max_size)
        .build(manager)
        .expect("Failed to build Postgres connection pool");

    ConnectionPool { pool }
}
