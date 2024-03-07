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

    /// 注册新用户，并返回该用户。若用户已经存在，则直接返回用户。
    pub fn register(&self, user_name: &str) -> Result<Guest, Box<dyn std::error::Error>> {
        use self::schema::guests::dsl::*;

        // 该用户是否已经存在？
        if let Some(user) = self.get_user(user_name)? {
            return Ok(user);
        };

        // 插入该数据
        let connection = &mut self.connections.get()?;
        let timestamp = Utc::now().naive_utc();
        let new_guest = NewGuest {
            name: user_name,
            credit: 0.0,
            created_at: timestamp,
            updated_at: timestamp,
        };

        // 返回结果
        Ok(diesel::insert_into(guests)
            .values(&new_guest)
            .returning(Guest::as_returning())
            .get_result(connection)?)
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

    /// 更新用户余额，并返回更新后的结果。
    pub fn update(&self, user: &Guest, cost: f64) -> Result<Guest, Box<dyn std::error::Error>> {
        use self::schema::guests::dsl::*;
        let connection = &mut self.connections.get()?;
        let post_guest = diesel::update(guests.find(user.id))
            .set((
                credit.eq(credit + cost),
                updated_at.eq(Utc::now().naive_utc()),
            ))
            .returning(Guest::as_returning())
            .get_result(connection)?;
        Ok(post_guest)
    }

    /// 删除用户
    pub fn remove_user(&self, by_name: &str) -> Result<usize, Box<dyn std::error::Error>> {
        use self::schema::guests::dsl::*;
        let user = self.get_user(by_name)?;
        if user.is_none() {
            return Err(Box::new(error::Error::new(format!(
                "Can not find user with name `{by_name}`"
            ))));
        }
        let connection = &mut self.connections.get()?;
        let num_deleted = diesel::delete(guests.find(user.unwrap().id))
            .execute(connection)
            .expect("User should be deleted without error");
        Ok(num_deleted)
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

#[cfg(test)]
mod tests {
    use super::DBAgent;
    #[test]
    fn test_db_init() {
        let agent = DBAgent::new(":memory:");
        assert!(agent.is_ok());
    }

    #[test]
    fn test_user_register() {
        let agent = DBAgent::new(":memory:").expect("Database agent should be initialized");

        // Register new users
        let user1 = agent
            .register("yinguobing")
            .expect("User registration should succeed");
        let _ = agent
            .register("robin")
            .expect("User registration should succeed");

        // Fetch the users
        let registered_user = agent
            .get_user("yinguobing")
            .expect("Existing user should be got without any error")
            .unwrap();

        assert_eq!(registered_user.created_at, registered_user.updated_at);
        assert_eq!(user1, registered_user);
    }

    #[test]
    fn test_duplicate_user_register() {
        let agent = DBAgent::new(":memory:").expect("Database agent should be initialized");

        // Register new users
        let user1 = agent
            .register("yinguobing")
            .expect("User registration should succeed");
        let user2 = agent
            .register("yinguobing")
            .expect("User registration should succeed");

        assert_eq!(user1, user2);
    }

    #[test]
    fn test_invalid_user_fetch() {
        let agent = DBAgent::new(":memory:").expect("Database agent should be initialized");
        // Fetch an invalid user
        let registered_user = agent
            .get_user("yinguobing")
            .expect("Existing user should be got without any error");
        assert_eq!(registered_user, None);
    }

    #[test]
    fn test_update_user_credit() {
        let agent = DBAgent::new(":memory:").expect("Database agent should be initialized");
        let user = agent
            .register("yinguobing")
            .expect("User registration should succeed");
        let _ = agent
            .update(&user, 42.0)
            .expect("User update should succeed");
        let post_user = agent
            .update(&user, -3.14)
            .expect("User update should succeed");
        assert_eq!(user.credit + 42.0 - 3.14, post_user.credit);
        assert_ne!(post_user.updated_at, post_user.created_at);
    }

    #[test]
    fn test_remove_user() {
        let agent = DBAgent::new(":memory:").expect("Database agent should be initialized");
        let _ = agent
            .register("yinguobing")
            .expect("User registration should succeed");
        let _ = agent
            .register("robin")
            .expect("User registration should succeed");
        let del_count = agent
            .remove_user("yinguobing")
            .expect("User should be removed without error");
        assert_eq!(del_count, 1);
        assert_eq!(agent.get_user("yinguobing").unwrap(), None);
    }
}
