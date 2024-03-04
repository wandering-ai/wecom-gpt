//! DBAgent负责将数据写入与读出数据库
use duckdb::DuckdbConnectionManager;
use r2d2::Pool;

pub struct DBAgent {
    path: String,
    connections: Pool<DuckdbConnectionManager>,
}

impl DBAgent {
    /// 创建新数据库
    pub fn new(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let manager = DuckdbConnectionManager::file(path)?;
        let pool = r2d2::Pool::new(manager)?;
        Ok(Self {
            path: path.to_owned(),
            connections: pool,
        })
    }

    /// 根据用户名，返回用户ID。
    pub fn get_user_id(&self, user_name: &str) -> Result<Option<i64>, Box<dyn std::error::Error>> {
        let db = self.connections.get()?;
        let sql = format!("SELECT id, name FROM guests WHERE name = user_name");
        let mut stmt = db.prepare(&sql)?;
        let query_result = stmt.query_map([], |row| Ok((row.get(0), row.get(1))))?;

        match query_result.into_iter().find(|x| {
            x.as_ref()
                .is_ok_and(|x| x.1.as_ref().is_ok_and(|u: &String| u == user_name))
        }) {
            Some(x) => Ok(Some(x.unwrap().0.unwrap())),
            None => Ok(None),
        }
    }

    /// 注册新用户，返回用户ID。
    pub fn register(&self, user_name: &str) -> Result<Option<i64>, Box<dyn std::error::Error>> {
        Ok(Some(0))
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
