//! DBAgent负责将数据写入与读出数据库
use duckdb::{params, DuckdbConnectionManager};
use r2d2::Pool;

pub struct DBAgent {
    connections: Pool<DuckdbConnectionManager>,
}

impl DBAgent {
    /// 创建新数据库
    pub fn new(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let manager = DuckdbConnectionManager::file(path)?;
        let pool = r2d2::Pool::new(manager)?;
        Ok(Self { connections: pool })
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

    /// 注册新用户，返回用户ID。若用户已经存在，则直接返回用户ID。
    pub fn register(
        &self,
        user_name: &str,
        credit: f64,
    ) -> Result<i64, Box<dyn std::error::Error>> {
        // 该用户是否已经存在？
        if let Some(guest_id) = self.get_user_id(user_name)? {
            return Ok(guest_id);
        };

        // 插入该数据
        let db = self.connections.get()?;
        let sql = format!("INSERT INTO guests (name, credit) VALUES (?, ?)");
        db.execute(&sql, params![user_name, credit])?;

        // 确认并返回ID
        let mut stmt = db.prepare("SELECT id FROM guests WHERE name = ?")?;
        let rows = stmt.query_and_then([user_name], |row| row.get::<_, i64>(0))?;
        let ids = rows
            .into_iter()
            .filter(|x| x.is_ok())
            .map(|x| x.unwrap())
            .collect::<Vec<i64>>();
        if ids.is_empty() {
            return Err(Box::new(error::Error::new(format!(
                "添加新用户到数据库失败。无法找到该用户：{}",
                user_name
            ))));
        }

        Ok(*ids.get(0).unwrap())
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
