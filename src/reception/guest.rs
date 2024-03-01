use std::time::{SystemTime, SystemTimeError, UNIX_EPOCH};
use uuid::Uuid;

// 与语言模型交互的用户信息
#[derive(Clone)]
pub struct Guest {
    id: String,
    name: String,
    credit: f64,
    create_time: f64,
    history: Vec<Message>,
    active_index: usize,
}

impl Guest {
    pub fn new(name: &str, credit: f64) -> Result<Self, SystemTimeError> {
        Ok(Self {
            id: Uuid::new_v4().to_string(),
            name: name.to_owned(),
            credit,
            create_time: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs_f64(),
            history: Vec::new(),
            active_index: 0,
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn credit(&self) -> f64 {
        self.credit
    }

    pub fn get_conversation(&self) -> Conversation {
        Conversation::new(&self.history[self.active_index..])
    }

    pub fn close_conversation(&mut self) {
        self.active_index = self.history.len();
    }

    /// 添加一条会话消息，同时更新用户账户余额
    pub fn append_message(&mut self, msg: Message) {
        self.history.push(msg.clone());
        self.credit -= msg.cost();
    }
}

// 通用用户会话。与特定语言模型无关。
pub struct Conversation<'a> {
    messages: &'a [Message],
}

impl<'b> Conversation<'b> {
    // 新建一个会话对象
    pub fn new(messages: &'b [Message]) -> Self {
        Self { messages }
    }

    // 获取全部消息的迭代器
    pub fn iter(&self) -> std::slice::Iter<'_, Message> {
        self.messages.iter()
    }

    // 获取当前会话长度
    pub fn len(&self) -> usize {
        self.messages.len()
    }

    // 获取当前会话总成本
    pub fn cost(&self) -> f64 {
        self.messages.iter().fold(0.0, |acc, x| acc + x.cost())
    }
}

// 会话记录中的一条消息
#[derive(Clone)]
pub struct Message {
    content: String,
    embedding: Option<Vec<u8>>,
    cost: f64,
    timestamp: f64,
    role: MessageRole,
}

// 消息发送者的角色
#[derive(Clone)]
pub enum MessageRole {
    System,
    User,
    Assistant,
}

impl Message {
    pub fn new(
        content: &str,
        embedding: &Option<Vec<u8>>,
        cost: f64,
        role: MessageRole,
    ) -> Result<Self, SystemTimeError> {
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs_f64();
        Ok(Self {
            content: content.to_owned(),
            embedding: embedding.to_owned(),
            cost,
            timestamp,
            role,
        })
    }

    // 获取消息角色
    pub fn role(&self) -> &MessageRole {
        &self.role
    }

    // 获取消息内容
    pub fn content(&self) -> &str {
        &self.content
    }

    // 获取消息成本
    pub fn cost(&self) -> f64 {
        self.cost
    }
}
