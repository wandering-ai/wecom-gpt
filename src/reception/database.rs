//! DBAgent负责将数据写入与读出数据库
mod models;
mod schema;

use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::sqlite::SqliteConnection;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

use chrono::Utc;
use models::{
    Assistant, ContentType, Conversation, Guest, MessageType, NewConversation, NewGuest, Provider,
};

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

        // 初始化数据库结构
        {
            let conn = &mut connections.get()?;
            conn.run_pending_migrations(MIGRATIONS)?;
        }

        // 填充AI供应商
        {
            let conn = &mut connections.get()?;
            let current_providers = vec![schema::providers::name.eq("openai/gpt4-32k")];
            diesel::insert_into(schema::providers::table)
                .values(&current_providers)
                .execute(conn)?;
        }

        // 填充AI助手
        {
            use schema::assistants::dsl::*;
            let conn = &mut connections.get()?;
            let current_assistants =
                vec![(name.eq("小白"), agent_id.eq(1000002), provider_id.eq(1))];
            diesel::insert_into(assistants)
                .values(&current_assistants)
                .execute(conn)?;
        }

        // 填充消息类型
        {
            use schema::msg_types::dsl::*;
            let conn = &mut connections.get()?;
            let message_types = vec![name.eq("system"), name.eq("user"), name.eq("assistant")];
            diesel::insert_into(msg_types)
                .values(&message_types)
                .execute(conn)?;
        }

        // 填充消息内容类型
        {
            use schema::content_types::dsl::*;
            let conn = &mut connections.get()?;
            let cnt_types = vec![
                name.eq("text"),
                name.eq("image"),
                name.eq("voice"),
                name.eq("video"),
                name.eq("file"),
                name.eq("markdown"),
                name.eq("news"),
                name.eq("textcard"),
            ];
            diesel::insert_into(content_types)
                .values(&cnt_types)
                .execute(conn)?;
        }

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
    pub fn update_user(
        &self,
        user: &Guest,
        cost: f64,
    ) -> Result<Guest, Box<dyn std::error::Error>> {
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

    /// 获取AI供应商信息
    pub fn get_ai_providers(&self) -> Result<Vec<Provider>, Box<dyn std::error::Error>> {
        use self::schema::providers::dsl::*;
        let conn = &mut self.connections.get()?;
        Ok(providers.load(conn)?)
    }

    /// 获取AI助手
    pub fn get_assistant(&self, by_id: i32) -> Result<Assistant, Box<dyn std::error::Error>> {
        use self::schema::assistants::dsl::*;
        let conn = &mut self.connections.get()?;
        Ok(assistants.find(by_id).first(conn)?)
    }

    /// 获取AI助手 - 通过AgentID
    pub fn get_assistant_by_agent_id(
        &self,
        by_agent_id: i32,
    ) -> Result<Assistant, Box<dyn std::error::Error>> {
        use self::schema::assistants::dsl::*;
        let conn = &mut self.connections.get()?;
        Ok(assistants
            .filter(agent_id.eq(by_agent_id))
            .select(Assistant::as_select())
            .first(conn)?)
    }

    /// 获取消息角色
    pub fn get_msg_types(&self) -> Result<Vec<MessageType>, Box<dyn std::error::Error>> {
        use self::schema::msg_types::dsl::*;
        let conn = &mut self.connections.get()?;
        Ok(msg_types.load(conn)?)
    }

    /// 获取消息内容类型
    pub fn get_content_types(&self) -> Result<Vec<ContentType>, Box<dyn std::error::Error>> {
        use self::schema::content_types::dsl::*;
        let conn = &mut self.connections.get()?;
        Ok(content_types.load(conn)?)
    }

    /// 创建会话记录
    pub fn create_conversation(
        &self,
        for_user: &Guest,
        with_assistant: &Assistant,
    ) -> Result<Conversation, Box<dyn std::error::Error>> {
        use schema::conversations::dsl::*;
        let timestamp = Utc::now().naive_utc();

        // Deactivate any existing active conversation
        {
            let existing_convs = Conversation::belonging_to(for_user).filter(active.eq(true));
            let conn = &mut self.connections.get()?;
            diesel::update(existing_convs)
                .set((active.eq(false), updated_at.eq(timestamp)))
                .execute(conn)?;
        }

        // Insert new one
        {
            let new_conv = NewConversation {
                guest_id: for_user.id,
                assistant_id: with_assistant.id,
                active: true,
                created_at: timestamp,
                updated_at: timestamp,
            };
            let conn = &mut self.connections.get()?;
            Ok(diesel::insert_into(conversations)
                .values(&new_conv)
                .returning(Conversation::as_returning())
                .get_result(conn)?)
        }
    }

    /// 按照ID获取会话记录
    pub fn get_conversation(&self, by_id: i32) -> Result<Conversation, Box<dyn std::error::Error>> {
        use schema::conversations::dsl::*;
        let conn = &mut self.connections.get()?;
        Ok(conversations.find(by_id).first(conn)?)
    }

    /// 获取用户当前活跃的会话记录
    pub fn get_active_conversation(
        &self,
        by_user: &Guest,
    ) -> Result<Conversation, Box<dyn std::error::Error>> {
        use schema::conversations::dsl::*;
        let conn = &mut self.connections.get()?;
        Ok(Conversation::belonging_to(by_user)
            .filter(active.eq(true))
            .first(conn)?)
    }

    /// 删除会话记录。返回本次删除会话记录的个数。
    pub fn remove_conversation(&self, by_id: i32) -> Result<usize, Box<dyn std::error::Error>> {
        use schema::conversations::dsl::*;
        let conn = &mut self.connections.get()?;
        Ok(diesel::delete(conversations.find(by_id)).execute(conn)?)
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
    use super::Provider;
    #[test]
    fn test_db_init() {
        let agent = DBAgent::new(":memory:");
        assert!(agent.is_ok());
        assert_eq!(
            agent.unwrap().get_ai_providers().unwrap(),
            vec![Provider {
                id: 1,
                name: "openai/gpt4-32k".to_string()
            }]
        )
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
            .update_user(&user, 42.0)
            .expect("User update should succeed");
        let post_user = agent
            .update_user(&user, -3.14)
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

    // 测试会话记录
    #[test]
    fn test_conversation() {
        let agent = DBAgent::new(":memory:").expect("Database agent should be initialized");

        let guest = agent
            .register("yinguobing")
            .expect("User registration should not fail");
        let assistant = agent
            .get_assistant(1)
            .expect("At least one assistant should be ready by default");

        // Create
        let conv1 = agent
            .create_conversation(&guest, &assistant)
            .expect("Conversation should be created without error");
        let conv2 = agent
            .create_conversation(&guest, &assistant)
            .expect("Conversation should be created without error");

        // Get active conversation
        let active_conv = agent
            .get_active_conversation(&guest)
            .expect("Active conversation should always be ready");

        assert_ne!(active_conv.updated_at, conv1.updated_at);
        assert_eq!(active_conv.updated_at, conv2.updated_at);

        // Delete old conversation
        assert_eq!(
            agent
                .remove_conversation(1)
                .expect("Conversation should be removed without error"),
            1
        );
        assert!(agent.get_conversation(1).is_err());
        assert!(agent.get_active_conversation(&guest).unwrap().active);
    }

    // 测试助手的初始化结果
    #[test]
    fn test_assistant_init() {
        let agent = DBAgent::new(":memory:").expect("Database agent should be initialized");
        let assistant = agent.get_assistant_by_agent_id(1000002).unwrap();
        assert_eq!(assistant.id, 1);
        assert_eq!(assistant.name, "小白");
    }

    // 测试消息角色类型的初始化结果
    #[test]
    fn test_msg_types_init() {
        let agent = DBAgent::new(":memory:").expect("Database agent should be initialized");
        let msg_types = agent.get_msg_types().unwrap();
        assert_eq!(
            vec![
                super::MessageType {
                    id: 1,
                    name: "system".to_string(),
                },
                super::MessageType {
                    id: 2,
                    name: "user".to_string(),
                },
                super::MessageType {
                    id: 3,
                    name: "assistant".to_string(),
                }
            ],
            msg_types
        );
    }
}
