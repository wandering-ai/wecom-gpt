//! DBAgent负责将数据写入与读出数据库
mod models;
mod schema;

use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::sqlite::SqliteConnection;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

use chrono::Utc;
use models::{Guest, NewGuest};

pub struct DBAgent {
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
        Ok(Self { connections })
    }

    /// 根据用户名获取用户。企业微信用户名具备唯一性。
    pub fn get_user(&self, by_name: &str) -> Result<Option<Guest>, Box<dyn std::error::Error>> {
        use self::schema::guests::dsl::*;
        let connection = &mut self.connections.get()?;
        Ok(guests
            .filter(name.eq(by_name))
            .select(Guest::as_select())
            .first(connection)
            .optional()?)
    }

    /// 注册新用户，并返回该用户。若用户已经存在，则直接返回用户。
    pub fn register(
        &self,
        user_name: &str,
        initial_credit: f64,
    ) -> Result<Guest, Box<dyn std::error::Error>> {
        use self::schema::guests::dsl::*;

        // 该用户是否已经存在？
        if let Some(user) = self.get_user(user_name)? {
            return Ok(user);
        };

        // 插入该数据
        let connection = &mut self.connections.get()?;
        let new_guest = NewGuest {
            name: user_name,
            credit: initial_credit,
            created_at: Utc::now().naive_utc(),
            updated_at: Utc::now().naive_utc(),
        };

        // 返回结果
        Ok(diesel::insert_into(guests)
            .values(&new_guest)
            .returning(Guest::as_returning())
            .get_result(connection)?)
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
