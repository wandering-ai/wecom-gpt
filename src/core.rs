/// 定义了系统运行所需的核心实体类型以及组合模块需要遵循的行为协议
use serde::{Deserialize, Serialize};
use std::error::Error;

/// 消息内容的类型
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum ContentType {
    Text,
    Image,
    Audio,
    Video,
    File,
}

impl ContentType {
    pub fn to_id(&self) -> i32 {
        match self {
            Self::Text => 1,
            Self::Image => 2,
            Self::Audio => 3,
            Self::Video => 4,
            Self::File => 5,
        }
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

/// 一条响应消息应当具备的行为
pub trait ChatResponse {
    /// 获取回复消息的文本内容
    fn content(&self) -> &str;
    // 本次响应消息的成本
    fn cost(&self) -> f64;
}

/// 提供聊天功能的对象应当具备的行为
pub trait Chat {
    // 根据用户与消息内容做出消息反馈
    async fn chat(
        &self,
        guest: &Guest,
        message: &str,
    ) -> Result<impl ChatResponse, Box<dyn Error + Send + Sync>>;

    // 返回用户当前会话的资源消耗
    fn audit(&self, guest: &Guest) -> String;

    // 开启新会话
    fn new_conversation(&self, guest: &Guest) -> Result<(), Box<dyn Error + Send + Sync>>;
}
