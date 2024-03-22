use chrono::Utc;
use std::fmt;

use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::sqlite::SqliteConnection;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};

use super::{model, schema};
use crate::core;
use crate::provider::openai;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

#[derive(Debug, Clone)]
pub enum Error {
    NotFound,
    Database(String),
    Connection(String),
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let err_msg = match self {
            Self::NotFound => "Item not found",
            Self::Database(msg) => msg,
            Self::Connection(msg) => msg,
        };
        write!(f, "{}", err_msg)
    }
}
impl std::error::Error for Error {}

pub struct Agent {
    connections: Pool<ConnectionManager<SqliteConnection>>,
}

impl Agent {
    /// 初始化数据库
    pub fn new(database_url: &str, admin: &str) -> Result<Self, Error> {
        // Init a db pool
        let manager = ConnectionManager::<SqliteConnection>::new(database_url);
        let connections = Pool::builder()
            .build(manager)
            .map_err(|e| Error::Database(e.to_string()))?;

        // 初始化数据库结构
        {
            let conn = &mut connections
                .get()
                .map_err(|e| Error::Connection(e.to_string()))?;
            conn.run_pending_migrations(MIGRATIONS)
                .map_err(|e| Error::Database(e.to_string()))?;
        }

        // 数据库默认内容需要初始化？
        let db_initialized: bool = {
            let conn = &mut connections
                .get()
                .map_err(|e| Error::Connection(e.to_string()))?;
            match schema::db_init_status::table
                .find(1)
                .first::<model::DbStatus>(conn)
            {
                Ok(o) => {
                    tracing::info!("当前数据库初始化于{}", o.initialized_at);
                    true
                }
                Err(e) => {
                    tracing::warn!("数据库尚未初始化（{e}）。将初始化数据库。");
                    false
                }
            }
        };
        if !db_initialized {
            let timestamp = Utc::now().naive_utc();
            // 填充默认的管理员用户
            {
                use schema::guests;
                let conn = &mut connections
                    .get()
                    .map_err(|e| Error::Connection(e.to_string()))?;
                diesel::insert_into(guests::table)
                    .values((
                        guests::id.eq(1),
                        guests::name.eq(admin),
                        guests::credit.eq(0.0),
                        guests::created_at.eq(timestamp),
                        guests::updated_at.eq(timestamp),
                        guests::admin.eq(true),
                    ))
                    .execute(conn)
                    .map_err(|e| Error::Database(format!("创建管理员账户出错。{e}")))?;
            }

            // 填充数据库初始化日期
            {
                use schema::db_init_status::dsl::*;
                let conn = &mut connections
                    .get()
                    .map_err(|e| Error::Connection(e.to_string()))?;
                diesel::insert_into(db_init_status)
                    .values(initialized_at.eq(timestamp))
                    .execute(conn)
                    .map_err(|e| Error::Database(e.to_string()))?;
            }
            tracing::info!("数据库初始化完成。");
        }

        Ok(Self { connections })
    }

    /// 注册新用户
    pub fn create_user(&self, guest: &core::Guest) -> Result<(), Error> {
        use self::schema::guests::dsl::*;

        // 插入该数据
        let conn = &mut self
            .connections
            .get()
            .map_err(|e| Error::Connection(e.to_string()))?;
        let timestamp = Utc::now().naive_utc();
        let new_guest = model::NewGuest {
            name: &guest.name,
            credit: guest.credit,
            created_at: timestamp,
            updated_at: timestamp,
            admin: guest.admin,
        };

        // 返回结果
        let _ = diesel::insert_into(guests)
            .values(&new_guest)
            .execute(conn)
            .map_err(|e| Error::Database(e.to_string()))?;
        Ok(())
    }

    /// 获取全部用户
    pub fn get_users(&self) -> Result<Vec<core::Guest>, Error> {
        use self::schema::guests::dsl::*;
        let conn = &mut self
            .connections
            .get()
            .map_err(|e| Error::Connection(e.to_string()))?;
        let db_users: Vec<model::Guest> = guests
            .load(conn)
            .map_err(|e| Error::Database(e.to_string()))?;
        let users = db_users
            .iter()
            .map(|u| core::Guest {
                name: u.name.clone(),
                credit: u.credit,
                admin: u.admin,
            })
            .collect();
        Ok(users)
    }

    /// 按照用户名获取用户
    pub fn get_user(&self, unique_guest_name: &str) -> Result<core::Guest, Error> {
        use self::schema::guests::dsl::*;
        let conn = &mut self
            .connections
            .get()
            .map_err(|e| Error::Connection(e.to_string()))?;
        let user: model::Guest = guests
            .filter(name.eq(unique_guest_name))
            .select(model::Guest::as_select())
            .first(conn)
            .map_err(|_| Error::NotFound)?;
        Ok(core::Guest {
            name: user.name,
            credit: user.credit,
            admin: user.admin,
        })
    }

    // 更新用户
    pub fn update_user(&self, guest: &core::Guest) -> Result<(), Error> {
        use self::schema::guests::dsl::*;
        let conn = &mut self
            .connections
            .get()
            .map_err(|e| Error::Connection(e.to_string()))?;
        diesel::update(guests.filter(name.eq(&guest.name)))
            .set((
                credit.eq(guest.credit),
                updated_at.eq(Utc::now().naive_utc()),
                admin.eq(guest.admin),
            ))
            .execute(conn)
            .map_err(|e| Error::Database(e.to_string()))?;
        Ok(())
    }

    // 新建一条会话记录作为当前活跃会话记录。
    // 此操作会将之前活跃会话记录标记为非活跃。
    pub fn create_conversation(&self, guest: &core::Guest, assistant_id: u64) -> Result<(), Error> {
        use schema::conversations;
        let timestamp = Utc::now().naive_utc();

        // Find the user
        let user: model::Guest = {
            use self::schema::guests::dsl::*;
            let conn = &mut self
                .connections
                .get()
                .map_err(|e| Error::Connection(e.to_string()))?;
            guests
                .filter(name.eq(&guest.name))
                .select(model::Guest::as_select())
                .first(conn)
                .map_err(|e| Error::Database(e.to_string()))?
        };

        // Deactivate any existing active conversation
        {
            let existing_convs = model::Conversation::belonging_to(&user)
                .filter(conversations::active.eq(true))
                .filter(conversations::assistant_id.eq(assistant_id as i32));
            let conn = &mut self
                .connections
                .get()
                .map_err(|e| Error::Connection(e.to_string()))?;
            diesel::update(existing_convs)
                .set((
                    conversations::active.eq(false),
                    conversations::updated_at.eq(timestamp),
                ))
                .execute(conn)
                .map_err(|e| Error::Database(e.to_string()))?;
        }

        // Insert new one
        {
            let new_conv = model::NewConversation {
                guest_id: user.id,
                assistant_id: assistant_id as i32,
                active: true,
                created_at: timestamp,
                updated_at: timestamp,
            };
            let conn = &mut self
                .connections
                .get()
                .map_err(|e| Error::Connection(e.to_string()))?;
            diesel::insert_into(conversations::table)
                .values(&new_conv)
                .execute(conn)
                .map_err(|e| Error::Database(e.to_string()))?;
        }
        Ok(())
    }

    /// 获取用户当前活跃的会话记录
    pub fn get_conversation(
        &self,
        guest: &core::Guest,
        assistant_id: u64,
    ) -> Result<Vec<model::Message>, Error> {
        // Find the user
        let user: model::Guest = {
            use self::schema::guests::dsl::*;
            let conn = &mut self
                .connections
                .get()
                .map_err(|e| Error::Connection(e.to_string()))?;
            guests
                .filter(name.eq(&guest.name))
                .select(model::Guest::as_select())
                .first(conn)
                .map_err(|e| Error::Database(e.to_string()))?
        };

        // Find the activate conversation
        let db_conv: model::Conversation = {
            use schema::conversations;
            let conn = &mut self
                .connections
                .get()
                .map_err(|e| Error::Connection(e.to_string()))?;
            model::Conversation::belonging_to(&user)
                .filter(conversations::active.eq(true))
                .filter(conversations::assistant_id.eq(assistant_id as i32))
                .first(conn)
                .map_err(|e| Error::Database(e.to_string()))?
        };

        // Find all the messages belonging to this conversation
        let messages: Vec<model::Message> = {
            let conn = &mut self
                .connections
                .get()
                .map_err(|e| Error::Connection(e.to_string()))?;
            let mut db_msgs: Vec<model::Message> = model::Message::belonging_to(&db_conv)
                .select(model::Message::as_select())
                .load(conn)
                .map_err(|e| Error::Database(e.to_string()))?;
            db_msgs.sort_by(|a, b| a.created_at.cmp(&b.created_at));
            db_msgs
        };
        Ok(messages)
    }

    // 将新的消息添加到用户当前会话内容结尾
    pub fn append_message(
        &self,
        guest: &core::Guest,
        assistant_id: u64,
        message: &openai::Message,
        cost: f64,
        prompt_tokens: u64,
        completion_tokens: u64,
    ) -> Result<(), Error> {
        // 获取当前用户
        let user = {
            use self::schema::guests::dsl::*;
            let conn = &mut self
                .connections
                .get()
                .map_err(|e| Error::Connection(e.to_string()))?;
            guests
                .filter(name.eq(&guest.name))
                .select(model::Guest::as_select())
                .first(conn)
                .map_err(|_| Error::NotFound)?
        };

        // 获取当前活跃会话
        let db_conv: model::Conversation = {
            use schema::conversations;
            let conn = &mut self
                .connections
                .get()
                .map_err(|e| Error::Connection(e.to_string()))?;
            model::Conversation::belonging_to(&user)
                .filter(conversations::active.eq(true))
                .filter(conversations::assistant_id.eq(assistant_id as i32))
                .first(conn)
                .map_err(|_| Error::NotFound)?
        };

        // 新增消息记录
        let timestamp = Utc::now().naive_utc();
        let new_msg = model::NewMessage {
            conversation_id: db_conv.id,
            created_at: timestamp,
            content: message.content.clone(),
            cost,
            message_type: openai::Role::try_from(message.role.as_str())
                .unwrap()
                .to_id(),
            content_type: core::ContentType::Text.to_id(), // Static for now
            prompt_tokens: prompt_tokens as i32,
            completion_tokens: completion_tokens as i32,
        };
        {
            use schema::messages;
            let conn = &mut self
                .connections
                .get()
                .map_err(|e| Error::Connection(e.to_string()))?;
            diesel::insert_into(messages::table)
                .values(&new_msg)
                .execute(conn)
                .map_err(|e| Error::Database(e.to_string()))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::Agent;

    // 测试默认ADMIN初始化
    #[test]
    fn test_init_user() {
        // 初始化
        let agent = Agent::new(":memory:", "administrator").expect("Agent init can not fail");
        assert_eq!(agent.get_user("administrator").unwrap().admin, true);
    }

    #[test]
    fn test_user_create() {
        use super::core;
        let agent =
            Agent::new(":memory:", "administrator").expect("Database agent should be initialized");

        // Register new users
        let guest = core::Guest {
            name: "yinguobing".to_string(),
            credit: 1.2,
            admin: true,
        };
        agent
            .create_user(&guest)
            .expect("User registration should succeed");

        // Fetch the users
        let registered_user = agent
            .get_user("yinguobing")
            .expect("Existing user should be got without any error");

        assert_eq!(guest, registered_user);
    }

    #[test]
    fn test_user_get_all() {
        use super::core;
        let agent =
            Agent::new(":memory:", "yinguobing").expect("Database agent should be initialized");

        // Register new users
        let guest = core::Guest {
            name: "robin".to_string(),
            credit: 1.2,
            admin: true,
        };
        agent
            .create_user(&guest)
            .expect("User registration should succeed");

        let admin = core::Guest {
            name: "yinguobing".to_string(),
            credit: 0.0,
            admin: true,
        };

        // Fetch the users
        let registered_users = agent
            .get_users()
            .expect("All existing user should be got without any error");

        assert_eq!(vec![admin, guest], registered_users);
    }

    #[test]
    fn test_user_duplicate_register() {
        use super::core;
        let agent =
            Agent::new(":memory:", "administrator").expect("Database agent should be initialized");

        // Register new users
        let guest = core::Guest {
            name: "yinguobing".to_string(),
            credit: 1.2,
            admin: true,
        };
        agent
            .create_user(&guest)
            .expect("User registration should succeed");
        assert!(agent.create_user(&guest).is_err());
    }

    #[test]
    fn test_user_invalid_get() {
        let agent =
            Agent::new(":memory:", "administrator").expect("Database agent should be initialized");
        // Fetch an invalid user
        assert!(agent.get_user("NotExisted").is_err());
    }

    #[test]
    fn test_user_update() {
        use super::core;
        let agent =
            Agent::new(":memory:", "administrator").expect("Database agent should be initialized");
        let mut guest = core::Guest {
            name: "yinguobing".to_string(),
            credit: 1.2,
            admin: true,
        };
        agent
            .create_user(&guest)
            .expect("User registration should succeed");
        guest.credit = 2.2;
        agent
            .update_user(&guest)
            .expect("User update should succeed");
        let user = agent.get_user(&guest.name).unwrap();
        assert_eq!(guest, user);
    }

    // 测试会话记录
    #[test]
    fn test_conversation() {
        use super::core;
        let agent =
            Agent::new(":memory:", "administrator").expect("Database agent should be initialized");

        let guest = core::Guest {
            name: "yinguobing".to_string(),
            credit: 1.2,
            admin: true,
        };
        agent
            .create_user(&guest)
            .expect("User registration should succeed");
        let assistant_id = 10003;

        // Create
        agent
            .create_conversation(&guest, assistant_id)
            .expect("1st Conversation should be created without error");
        let msg1 = super::openai::Message {
            content: "message a".to_string(),
            role: super::openai::Role::User.to_string(),
        };
        agent
            .append_message(&guest, assistant_id, &msg1, 0.18, 0, 0)
            .expect("Conversation should be updated without error");

        agent
            .create_conversation(&guest, assistant_id)
            .expect("Conversation should be created without error");
        let msg2 = super::openai::Message {
            content: "message b".to_string(),
            role: super::openai::Role::Assistant.to_string(),
        };
        agent
            .append_message(&guest, assistant_id, &msg2, 0.81, 2, 5)
            .expect("Conversation should be updated without error");

        // Get active conversation
        let active_conv = agent
            .get_conversation(&guest, assistant_id)
            .expect("Active conversation should always be ready");

        assert_eq!(
            super::openai::Message::from(active_conv.first().unwrap()),
            msg2
        );
    }
}
