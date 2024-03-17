/// 定义了系统运行所需的核心实体类型以及组合模块需要遵循的行为协议
use serde::{Deserialize, Serialize};
use std::error::Error;

/// 会话中消息发送者的角色
/// `System`系统角色，与对话的双方无关。
/// `User` 人类用户
/// `Assistant` 智能助手
/// `Supplementary` 非以上类型的其它角色
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum MessageRole {
    System,
    User,
    Assistant,
    Supplementary,
}

/// 消息内容的类型
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum ContentType {
    Text,
    Image,
    Audio,
    Video,
    File,
}

/// 一条消息
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Message {
    pub content: String,
    pub role: MessageRole,
    pub cost: f64,
    pub tokens: usize,
}

/// 一段对话
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Conversation {
    pub content: Vec<Message>,
}

impl Conversation {
    // 会话累计消耗的Credit
    pub fn cost(&self) -> f64 {
        self.content.iter().fold(0.0, |acc, x| acc + x.cost)
    }

    // 会话累计消耗的Token
    pub fn tokens(&self) -> usize {
        self.content.iter().fold(0, |acc, x| acc + x.tokens)
    }
}

/// 一名用户
/// 通常一名用户会有多段会话。当前简化问题，仅保留一段。
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Guest {
    pub name: String,
    pub credit: f64,
    pub admin: bool,
}

/// 一名助手，与用户直接对话的实体。
/// `agent_id`企业微信的应用ID
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Assistant {
    pub name: String,
    pub agent_id: usize,
}

/// 提供数据存储功能的对象应当具备的行为
pub trait PersistStore {
    // 新建用户
    fn create_user(&self, guest: &Guest) -> Result<(), Box<dyn Error + Send + Sync>>;
    // 获取用户
    fn get_user(&self, unique_guest_name: &str) -> Result<Guest, Box<dyn Error + Send + Sync>>;
    // 更新用户
    fn update_user(&self, guest: &Guest) -> Result<(), Box<dyn Error + Send + Sync>>;
    // 新建会话
    fn create_conversation(
        &self,
        guest: &Guest,
        assistant: &Assistant,
    ) -> Result<(), Box<dyn Error + Send + Sync>>;
    // 获取用户的活跃会话
    fn get_conversation(&self, guest: &Guest)
        -> Result<Conversation, Box<dyn Error + Send + Sync>>;
    // 将新的消息添加到用户当前会话内容结尾
    fn append_message(
        &self,
        guest: &Guest,
        message: &Message,
    ) -> Result<(), Box<dyn Error + Send + Sync>>;
    // 获取助手
    fn get_assistant_by_agent_id(&self, id: i32)
        -> Result<Assistant, Box<dyn Error + Send + Sync>>;
}

/// 一条响应消息应当具备的行为
pub trait ChatResponse {
    /// 获取回复消息的文本内容
    fn content(&self) -> &str;
    /// 获取回复消息的角色
    fn role(&self) -> MessageRole;
    // 本次响应消息的成本
    fn cost(&self) -> f64;
    // 本次响应消息的消耗
    fn tokens(&self) -> usize;
}

/// 提供聊天功能的对象应当具备的行为
pub trait Chat {
    // AI供应商应当能够根据会话内容返回消息
    async fn chat(
        &self,
        conversation: &Conversation,
    ) -> Result<impl ChatResponse, Box<dyn Error + Send + Sync>>;
}
