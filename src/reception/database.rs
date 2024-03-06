//! DBAgent负责将数据写入与读出数据库
mod schema;
use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::sqlite::SqliteConnection;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

pub struct DBAgent {
    database_url: String,
    connections: Pool<ConnectionManager<SqliteConnection>>,
}

impl DBAgent {
    /// 初始化数据库
    pub fn new(
        database_url: &str,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync + 'static>> {
        // Init a db pool
        let manager = ConnectionManager::<SqliteConnection>::new(database_url);
        let connections = Pool::builder().build(manager)?;

        // Init the database with diesel
        let mut conn = connections
            .get()
            .expect("DB connection should be fetched from pool");
        conn.run_pending_migrations(MIGRATIONS)?;
        Ok(Self {
            database_url: database_url.to_owned(),
            connections,
        })
    }
}

mod error {
    use std::fmt;

    #[derive(Debug, Clone)]
    pub struct Error {
        text: String,
    }

    impl Error {
        pub fn new(text: String) -> Self {
            Self { text }
        }
    }

    impl fmt::Display for Error {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "{}", self.text)
        }
    }

    impl std::error::Error for Error {}
}
