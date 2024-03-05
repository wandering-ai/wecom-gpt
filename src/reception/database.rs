//! DBAgent负责将数据写入与读出数据库
mod schema;

use r2d2::Pool;

pub struct User {
    id: i64,
    name: String,
    credit: f64,
    created_at: String,
    updated_at: String,
}

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

    /// 根据用户名获取用户
    pub fn get_user(&self, by_name: &str) -> Result<Option<User>, Box<dyn std::error::Error>> {
        let db = self.connections.get()?;
        let sql = format!(
            "SELECT id, name, credit, created_at, updated_at FROM guests WHERE name = user_name"
        );
        let mut stmt = db.prepare(&sql)?;

        let results = stmt.query_map([], |row| {
            Ok(User {
                id: row.get(0)?,
                name: row.get(1)?,
                credit: row.get(2)?,
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
            })
        })?;
        let users: Vec<_> = results.map(|x| x.unwrap()).collect();
        if users.is_empty() {
            return Ok(None);
        }
        Ok(Some(users[0]))
    }

    /// 注册新用户，返回用户ID。若用户已经存在，则直接返回用户ID。
    pub fn register(
        &self,
        user_name: &str,
        credit: f64,
    ) -> Result<User, Box<dyn std::error::Error>> {
        // 该用户是否已经存在？
        if let Some(user) = self.get_user(user_name)? {
            return Ok(user);
        };

        // 插入该数据
        let db = self.connections.get()?;
        let sql = format!(
            "INSERT INTO guests (name, credit, created_at, updated_at) VALUES (?, ?, ?, ?)"
        );
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
