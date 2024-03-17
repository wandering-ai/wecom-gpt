use super::{model, schema};
use crate::reception::core;
use chrono::Utc;
use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::sqlite::SqliteConnection;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use std::convert::From;
use std::env;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

// 助手类型转换
impl From<model::Assistant> for core::Assistant {
    fn from(value: model::Assistant) -> Self {
        Self {
            name: value.name,
            agent_id: value.agent_id as usize,
        }
    }
}

// 消息角色转换：core::Message -> model::Message
impl From<&core::MessageRole> for model::MessageType {
    fn from(value: &core::MessageRole) -> Self {
        let (id, name) = match value {
            core::MessageRole::System => (1, "system"),
            core::MessageRole::User => (2, "user"),
            core::MessageRole::Assistant => (3, "assistant"),
            core::MessageRole::Supplementary => (4, "supplementary"),
        };
        Self {
            id,
            name: name.to_string(),
        }
    }
}
impl From<core::MessageRole> for model::MessageType {
    fn from(value: core::MessageRole) -> Self {
        (&value).into()
    }
}

// 消息角色转换：model::Message -> core::Message
impl From<i32> for core::MessageRole {
    fn from(value: i32) -> Self {
        match value {
            1 => core::MessageRole::System,
            2 => core::MessageRole::User,
            3 => core::MessageRole::Assistant,
            _ => core::MessageRole::Supplementary,
        }
    }
}
impl From<&model::MessageType> for core::MessageRole {
    fn from(value: &model::MessageType) -> Self {
        value.id.into()
    }
}
impl From<model::MessageType> for core::MessageRole {
    fn from(value: model::MessageType) -> Self {
        (&value).into()
    }
}

// 内容类型转换：core::ContentType -> model::ContentType
impl From<&core::ContentType> for model::ContentType {
    fn from(value: &core::ContentType) -> Self {
        let (id, name) = match value {
            core::ContentType::Text => (1, "text"),
            core::ContentType::Image => (2, "image"),
            core::ContentType::Audio => (3, "audio"),
            core::ContentType::Video => (4, "video"),
            core::ContentType::File => (5, "file"),
        };
        model::ContentType {
            id,
            name: name.to_string(),
        }
    }
}
impl From<core::ContentType> for model::ContentType {
    fn from(value: core::ContentType) -> Self {
        (&value).into()
    }
}

// 消息类型转换：model::Message -> for core::Message
impl From<&model::Message> for core::Message {
    // 将数据库Message转换为核心类型
    fn from(value: &model::Message) -> Self {
        Self {
            content: value.content.clone(),
            role: value.message_type.into(),
            cost: value.cost,
            tokens: value.tokens as usize,
        }
    }
}
impl From<model::Message> for core::Message {
    // 将数据库Message转换为核心类型
    fn from(value: model::Message) -> Self {
        (&value).into()
    }
}

// 使用默认内容填充数据库。当数据库第一次初始化时使用。
fn default_init(
    connections: &Pool<ConnectionManager<SqliteConnection>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
    // 填充默认的管理员用户
    {
        use schema::guests;
        let admin = env::var("APP_ADMIN").expect("Environment variable $APP_ADMIN must be set");
        let timestamp = Utc::now().naive_utc();
        let conn = &mut connections.get()?;
        diesel::insert_into(guests::table)
            .values((
                guests::id.eq(1),
                guests::name.eq(admin),
                guests::credit.eq(0.0),
                guests::created_at.eq(timestamp),
                guests::updated_at.eq(timestamp),
                guests::admin.eq(true),
            ))
            .execute(conn)?;
    }

    // 填充AI供应商
    {
        use schema::providers;
        let conn = &mut connections.get()?;
        let current_providers = vec![(
            providers::name.eq("openai/gpt-4-32k"),
            providers::max_tokens.eq(32 * 1000),
            providers::endpoint.eq("https://ai-openai872806641955.openai.azure.com/openai/deployments/gpt-4-32k/chat/completions?api-version=2023-03-15-preview"),
            providers::prompt_token_price.eq(0.06),
            providers::completion_token_price.eq(0.12),
        )];
        diesel::insert_into(providers::table)
            .values(&current_providers)
            .execute(conn)?;
    }

    // 填充AI助手
    {
        use schema::assistants::dsl::*;
        let conn = &mut connections.get()?;
        let current_assistants = vec![(name.eq("小白"), agent_id.eq(1000002), provider_id.eq(1))];
        diesel::insert_into(assistants)
            .values(&current_assistants)
            .execute(conn)?;
    }

    // 填充消息类型
    {
        use schema::msg_types::dsl::*;
        let conn = &mut connections.get()?;
        let message_types = vec![
            name.eq("system"),
            name.eq("user"),
            name.eq("assistant"),
            name.eq("supplementary"),
        ];
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
            name.eq("audio"),
            name.eq("video"),
            name.eq("file"),
        ];
        diesel::insert_into(content_types)
            .values(&cnt_types)
            .execute(conn)?;
    }

    // 填充数据库初始化日期
    {
        use schema::db_init_status::dsl::*;
        let conn = &mut connections.get()?;
        diesel::insert_into(db_init_status)
            .values(initialized_at.eq(Utc::now().naive_utc()))
            .execute(conn)?;
    }

    Ok(())
}

pub struct Agent {
    connections: Pool<ConnectionManager<SqliteConnection>>,
}

impl Agent {
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

        // 数据库默认内容需要初始化？
        let db_initialized: bool;
        {
            let conn = &mut connections.get()?;
            db_initialized = match schema::db_init_status::table
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
            };
        }
        if !db_initialized {
            default_init(&connections)?;
            tracing::info!("数据库初始化完成。");
        }

        Ok(Self { connections })
    }

    /// 获取AI供应商信息 - 通过ID
    pub fn get_provider(
        &self,
        by_id: i32,
    ) -> Result<model::Provider, Box<dyn std::error::Error + Send + Sync>> {
        use self::schema::providers::dsl::*;
        let conn = &mut self.connections.get()?;
        Ok(providers.find(by_id).first(conn)?)
    }

    /// 获取AI助手的ID
    pub fn get_assistant_id_by_agent_id(
        &self,
        by_id: i32,
    ) -> Result<i32, Box<dyn std::error::Error + Send + Sync>> {
        use self::schema::assistants::dsl::*;
        let conn = &mut self.connections.get()?;
        Ok(assistants
            .filter(agent_id.eq(by_id))
            .select(id)
            .first(conn)?)
    }

    /// 获取全部消息角色
    #[allow(dead_code)]
    pub fn get_msg_types(
        &self,
    ) -> Result<Vec<model::MessageType>, Box<dyn std::error::Error + Send + Sync>> {
        use self::schema::msg_types::dsl::*;
        let conn = &mut self.connections.get()?;
        Ok(msg_types.load(conn)?)
    }

    /// 获取全部消息内容类型
    #[allow(dead_code)]
    pub fn get_content_types(
        &self,
    ) -> Result<Vec<model::ContentType>, Box<dyn std::error::Error + Send + Sync>> {
        use self::schema::content_types::dsl::*;
        let conn = &mut self.connections.get()?;
        Ok(content_types.load(conn)?)
    }
}

// 实现存储特性
impl core::PersistStore for Agent {
    /// 获取AI助手 - 通过AgentID
    fn get_assistant_by_agent_id(
        &self,
        by_id: i32,
    ) -> Result<core::Assistant, Box<dyn std::error::Error + Send + Sync>> {
        use self::schema::assistants::dsl::*;
        let conn = &mut self.connections.get()?;
        Ok(assistants
            .filter(agent_id.eq(by_id))
            .select(model::Assistant::as_select())
            .first(conn)?
            .into())
    }

    /// 注册新用户
    fn create_user(
        &self,
        guest: &core::Guest,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        use self::schema::guests::dsl::*;

        // 插入该数据
        let conn = &mut self.connections.get()?;
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
            .execute(conn)?;
        Ok(())
    }

    /// 按照用户名获取用户
    fn get_user(
        &self,
        unique_guest_name: &str,
    ) -> Result<core::Guest, Box<dyn std::error::Error + Send + Sync>> {
        use self::schema::guests::dsl::*;
        let conn = &mut self.connections.get()?;
        let user: model::Guest = guests
            .filter(name.eq(unique_guest_name))
            .select(model::Guest::as_select())
            .first(conn)?;
        Ok(core::Guest {
            name: user.name,
            credit: user.credit,
            admin: user.admin,
        })
    }

    // 更新用户
    fn update_user(
        &self,
        guest: &core::Guest,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        use self::schema::guests::dsl::*;
        let conn = &mut self.connections.get()?;
        diesel::update(guests.filter(name.eq(&guest.name)))
            .set((
                credit.eq(guest.credit),
                updated_at.eq(Utc::now().naive_utc()),
                admin.eq(guest.admin),
            ))
            .execute(conn)?;
        Ok(())
    }

    // 新建一条会话记录作为当前活跃会话记录。
    // 此操作会将之前活跃会话记录标记为非活跃。
    fn create_conversation(
        &self,
        guest: &core::Guest,
        assistant: &core::Assistant,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        use schema::conversations::dsl::*;
        let timestamp = Utc::now().naive_utc();

        // Find the user
        let user: model::Guest = {
            use self::schema::guests::dsl::*;
            let conn = &mut self.connections.get()?;
            guests
                .filter(name.eq(&guest.name))
                .select(model::Guest::as_select())
                .first(conn)?
        };

        // Deactivate any existing active conversation
        {
            let existing_convs = model::Conversation::belonging_to(&user).filter(active.eq(true));
            let conn = &mut self.connections.get()?;
            diesel::update(existing_convs)
                .set((active.eq(false), updated_at.eq(timestamp)))
                .execute(conn)?;
        }

        // Find the assistant
        let assist_id = self.get_assistant_id_by_agent_id(assistant.agent_id as i32)?;

        // Insert new one
        {
            let new_conv = model::NewConversation {
                guest_id: user.id,
                assistant_id: assist_id,
                active: true,
                created_at: timestamp,
                updated_at: timestamp,
            };
            let conn = &mut self.connections.get()?;
            diesel::insert_into(conversations)
                .values(&new_conv)
                .execute(conn)?;
        }
        Ok(())
    }

    /// 获取用户当前活跃的会话记录
    fn get_conversation(
        &self,
        guest: &core::Guest,
    ) -> Result<core::Conversation, Box<dyn std::error::Error + Send + Sync>> {
        // Find the user
        let user: model::Guest = {
            use self::schema::guests::dsl::*;
            let conn = &mut self.connections.get()?;
            guests
                .filter(name.eq(&guest.name))
                .select(model::Guest::as_select())
                .first(conn)?
        };

        // Find the activate conversation
        let db_conv: model::Conversation = {
            use schema::conversations::dsl::*;
            let conn = &mut self.connections.get()?;
            model::Conversation::belonging_to(&user)
                .filter(active.eq(true))
                .first(conn)?
        };

        // Find all the messages belonging to this conversation
        let content: Vec<core::Message> = {
            let conn = &mut self.connections.get()?;
            let mut db_msgs: Vec<model::Message> = model::Message::belonging_to(&db_conv)
                .select(model::Message::as_select())
                .load(conn)?;
            db_msgs.sort_by(|a, b| a.created_at.cmp(&b.created_at));
            db_msgs.iter().map(|x| x.into()).collect()
        };
        Ok(core::Conversation { content })
    }

    // 将新的消息添加到用户当前会话内容结尾
    fn append_message(
        &self,
        guest: &core::Guest,
        message: &core::Message,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // 获取当前用户
        let user = {
            use self::schema::guests::dsl::*;
            let conn = &mut self.connections.get()?;
            guests
                .filter(name.eq(&guest.name))
                .select(model::Guest::as_select())
                .first(conn)?
        };

        // 获取当前活跃会话
        let db_conv: model::Conversation = {
            use schema::conversations::dsl::*;
            let conn = &mut self.connections.get()?;
            model::Conversation::belonging_to(&user)
                .filter(active.eq(true))
                .first(conn)?
        };

        // 新增消息记录
        let timestamp = Utc::now().naive_utc();
        let new_msg = model::NewMessage {
            conversation_id: db_conv.id,
            created_at: timestamp,
            content: message.content.clone(),
            cost: message.cost,
            message_type: model::MessageType::from(&message.role).id,
            content_type: model::ContentType::from(core::ContentType::Text).id, // Static for now
            tokens: message.tokens as i32,
        };
        {
            use schema::messages;
            let conn = &mut self.connections.get()?;
            diesel::insert_into(messages::table)
                .values(&new_msg)
                .execute(conn)?;
        }
        Ok(())
    }
}

mod error {
    use std::fmt;

    #[derive(Debug, Clone)]
    pub struct NotFound;

    impl fmt::Display for NotFound {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "Item not found in database")
        }
    }

    impl std::error::Error for NotFound {}
}

#[cfg(test)]
mod tests {
    use super::model;
    use super::Agent;
    use crate::reception::core::PersistStore;

    // 测试默认ADMIN初始化
    #[test]
    fn test_init_user() {
        // 初始化
        std::env::set_var("APP_ADMIN", "administrator");
        let agent = Agent::new(":memory:").expect("Agent init can not fail");
        assert_eq!(agent.get_user("administrator").unwrap().admin, true);
    }

    // 默认Provider初始化
    #[test]
    fn test_init_provider() {
        // 初始化
        std::env::set_var("APP_ADMIN", "administrator");
        let agent = Agent::new(":memory:").expect("Agent init can not fail");
        assert_eq!(
                agent.get_provider(1).unwrap(),
                model::Provider {
                    id: 1,
                    name: "openai/gpt-4-32k".to_string(),
                    endpoint: "https://ai-openai872806641955.openai.azure.com/openai/deployments/gpt-4-32k/chat/completions?api-version=2023-03-15-preview".to_string(),
                    prompt_token_price: 0.06,
                    completion_token_price: 0.12,
                    max_tokens: 32 * 1000_i32,
                }
            );
    }
    // 测试助手的初始化结果
    #[test]
    fn test_init_assistant() {
        std::env::set_var("APP_ADMIN", "administrator");
        let agent = Agent::new(":memory:").expect("Database agent should be initialized");
        let assistant = agent.get_assistant_by_agent_id(1000002).unwrap();
        assert_eq!(assistant.name, "小白");
    }

    // 测试消息角色类型的初始化结果
    #[test]
    fn test_init_msg_type() {
        std::env::set_var("APP_ADMIN", "administrator");
        let agent = Agent::new(":memory:").expect("Database agent should be initialized");
        let msg_types = agent.get_msg_types().unwrap();
        assert_eq!(
            vec![
                model::MessageType {
                    id: 1,
                    name: "system".to_string(),
                },
                model::MessageType {
                    id: 2,
                    name: "user".to_string(),
                },
                model::MessageType {
                    id: 3,
                    name: "assistant".to_string(),
                },
                model::MessageType {
                    id: 4,
                    name: "supplementary".to_string(),
                }
            ],
            msg_types
        );
    }

    // 测试内容类型的初始化结果
    #[test]
    fn test_init_content_type() {
        std::env::set_var("APP_ADMIN", "administrator");
        let agent = Agent::new(":memory:").expect("Database agent should be initialized");
        let content_types = agent.get_content_types().unwrap();
        assert_eq!(
            vec![
                model::ContentType {
                    id: 1,
                    name: "text".to_string()
                },
                model::ContentType {
                    id: 2,
                    name: "image".to_string()
                },
                model::ContentType {
                    id: 3,
                    name: "audio".to_string()
                },
                model::ContentType {
                    id: 4,
                    name: "video".to_string()
                },
                model::ContentType {
                    id: 5,
                    name: "file".to_string()
                },
            ],
            content_types
        );
    }

    #[test]
    fn test_user_create() {
        use super::core;
        std::env::set_var("APP_ADMIN", "administrator");
        let agent = Agent::new(":memory:").expect("Database agent should be initialized");

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
    fn test_user_duplicate_register() {
        use super::core;
        std::env::set_var("APP_ADMIN", "administrator");
        let agent = Agent::new(":memory:").expect("Database agent should be initialized");

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
        std::env::set_var("APP_ADMIN", "administrator");
        let agent = Agent::new(":memory:").expect("Database agent should be initialized");
        // Fetch an invalid user
        assert!(agent.get_user("NotExisted").is_err());
    }

    #[test]
    fn test_user_update() {
        use super::core;
        std::env::set_var("APP_ADMIN", "administrator");
        let agent = Agent::new(":memory:").expect("Database agent should be initialized");
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
        std::env::set_var("APP_ADMIN", "administrator");
        let agent = Agent::new(":memory:").expect("Database agent should be initialized");

        let guest = core::Guest {
            name: "yinguobing".to_string(),
            credit: 1.2,
            admin: true,
        };
        agent
            .create_user(&guest)
            .expect("User registration should succeed");
        let assistant = agent
            .get_assistant_by_agent_id(1000002)
            .expect("At least one assistant should be ready by default");

        // Create
        agent
            .create_conversation(&guest, &assistant)
            .expect("Conversation should be created without error");
        let msg1 = core::Message {
            content: "message a".to_string(),
            role: core::MessageRole::User,
            cost: 0.0,
            tokens: 10,
        };
        agent
            .append_message(&guest, &msg1)
            .expect("Conversation should be updated without error");

        agent
            .create_conversation(&guest, &assistant)
            .expect("Conversation should be created without error");
        let msg2 = core::Message {
            content: "message b".to_string(),
            role: core::MessageRole::Assistant,
            cost: 1.0,
            tokens: 10,
        };
        agent.append_message(&guest, &msg2).unwrap();

        // Get active conversation
        let active_conv = agent
            .get_conversation(&guest)
            .expect("Active conversation should always be ready");

        assert_eq!(active_conv.content[0], msg2);
    }
}
