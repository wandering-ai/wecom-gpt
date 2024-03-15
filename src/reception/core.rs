use serde::{Deserialize, Serialize};
use std::error::Error;

/// 会话过程中涉及到的角色
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum MessageRole {
    #[serde(rename = "system")]
    System,
    #[serde(rename = "user")]
    User,
    #[serde(rename = "assistant")]
    Assistant,
    #[serde(rename = "supplementary")]
    Supplementary,
}

/// 一条消息应当具备的行为
pub trait AIMessage {
    /// 获取本条消息的文本内容
    fn content(&self) -> &str;
    /// 获取本条消息的角色
    fn role(&self) -> MessageRole;
    // 获取消息的成本
    fn cost(&self) -> f64;
}

/// 与AI的会话记录应当具备的行为
pub trait AIConversation {
    // 逐条枚举会话记录
    fn messages(&self) -> Vec<&impl AIMessage>;
}

/// 提供AI功能的供应商应当具备的行为
pub trait AIProvider {
    // AI供应商应当能够根据会话内容返回消息
    async fn chat<T>(
        &self,
        conversation: T,
    ) -> Result<impl AIMessage, Box<dyn Error + Send + Sync>>
    where
        T: AIConversation + Serialize;
}
