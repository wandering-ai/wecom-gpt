/// Assistant负责处理与用户的会话
/// - 纳管用户与自己的会话记录。
/// - 协调会话记录与后端AI供应商的兼容性。
pub use crate::provider::openai::Config as ProviderCfg;

use crate::core;
use crate::provider::openai::{Agent as AIAgent, Conversation, Message, Role};
use crate::storage::Agent as StorageAgent;
use serde::Deserialize;
use std::fmt;
use std::sync::Arc;
use tiktoken_rs::{cl100k_base, CoreBPE};

// Custom Error
#[derive(Debug, Clone)]
pub enum Error {
    StorageError(String),
    ProviderError(String),
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let err = match self {
            Self::StorageError(e) => format!("数据库错误。{e}"),
            Self::ProviderError(e) => format!("供应商错误。{e}"),
        };
        write!(f, "{}", err)
    }
}
impl std::error::Error for Error {}

/// 智能助手初始化所需要的参数
#[derive(Deserialize, Clone)]
pub struct Config {
    pub agent_id: u64,
    pub name: String,
    pub token: String,
    pub key: String,
    pub secret: String,
    pub prompt: String,
    pub provider_id: u64,
    pub context_tokens_reservation: u64,
}

/// 助手的回复
pub struct Response {
    content: String,
    cost: f64,
}

impl core::ChatResponse for Response {
    fn content(&self) -> &str {
        self.content.as_str()
    }
    fn cost(&self) -> f64 {
        self.cost
    }
}

/// Assistant根据当前用户与用户消息来生成合适的回复
pub struct Assistant {
    provider: AIAgent,
    storage: Arc<StorageAgent>,
    id: u64,
    prompt: String,
    context_tokens_reservation: u64,
    token_counter: CoreBPE,
}

impl Assistant {
    pub fn new(config: &Config, provider_cfg: &ProviderCfg, storage: Arc<StorageAgent>) -> Self {
        let provider = AIAgent::new(provider_cfg);
        Self {
            provider,
            storage,
            id: config.agent_id,
            prompt: config.prompt.clone(),
            context_tokens_reservation: config.context_tokens_reservation,
            token_counter: cl100k_base().unwrap(),
        }
    }
}

impl core::Chat for Assistant {
    /// 根据用户消息，返回合适的回复
    async fn chat(
        &self,
        guest: &core::Guest,
        message: &str,
    ) -> Result<Response, Box<dyn std::error::Error + Send + Sync>> {
        // 获取用户会话记录。若会话记录不存在，则创建新记录。
        if let Err(e) = self.storage.get_conversation(guest, self.id) {
            tracing::warn!(
                "获取用户{}会话记录失败：{}。将为此用户创建新记录。",
                guest.name,
                e
            );
            self.storage
                .create_conversation(guest, self.id)
                .map_err(|e| Error::StorageError(format!("创建会话记录失败。{e}")))?;
            tracing::info!("已为用户{}创建会话记录。", guest.name);
        };
        let db_conv = match self.storage.get_conversation(guest, self.id) {
            Err(e) => {
                return Err(Box::new(Error::StorageError(format!(
                    "获取会话记录失败。{e}"
                ))))
            }
            Ok(c) => c,
        };
        tracing::debug!("Got conversation with {} messages", db_conv.len());

        // 即将发送给AI的会话
        let mut oai_conv: Vec<Message> = Vec::new();

        // 追加用户消息
        let user_msg = Message {
            role: Role::User.to_string(),
            content: message.to_owned(),
        };
        oai_conv.push(user_msg.clone());

        // 填充历史会话。注意会话超长问题。
        let mut prompt_tokens: usize = 0;
        for t in db_conv.iter().enumerate().rev() {
            prompt_tokens += self
                .token_counter
                .encode_with_special_tokens(&t.1.content)
                .len();
            if prompt_tokens as u64 >= self.provider.max_tokens() - self.context_tokens_reservation
            {
                tracing::warn!("Conversation cut at index {}", t.0);
                break;
            }
            oai_conv.push(Message {
                role: Role::try_from(t.1.message_type)?.to_string(),
                content: t.1.content.clone(),
            })
        }
        tracing::debug!("Total messages to AI: {}", oai_conv.len());

        // 填充系统消息
        if oai_conv
            .first()
            .is_some_and(|m| m.role != Role::System.to_string())
        {
            oai_conv.push(Message {
                content: self.prompt.clone(),
                role: Role::System.to_string(),
            });
            tracing::warn!("System message not found, default used.")
        }

        // 恢复正常时序
        oai_conv.reverse();

        // 交由AI处理
        let ai_response = match self
            .provider
            .process(&Conversation { messages: oai_conv })
            .await
        {
            // 告知用户发生内部错误，避免用户徒劳重试或者等待
            Err(e) => {
                return Err(Box::new(Error::ProviderError(format!(
                    "获取AI回复时发生错误。{e}"
                ))))
            }
            Ok(r) => r,
        };
        tracing::debug!("AI replied");

        // 记录用户消息，并与当前会话记录关联
        if let Err(e) = self
            .storage
            .append_message(guest, self.id, &user_msg, 0.0, 0, 0)
        {
            return Err(Box::new(Error::StorageError(format!("追加消息失败。{e}"))));
        }
        tracing::debug!("User message appended");

        // 更新AI回复到会话记录
        tracing::debug!("Constructing reply message");
        let ai_reply = Message {
            role: ai_response.role().to_string(),
            content: ai_response.content().to_owned(),
        };
        let cost = self.provider.cost(&ai_response);
        if let Err(e) = self.storage.append_message(
            guest,
            self.id,
            &ai_reply,
            cost,
            ai_response.prompt_tokens(),
            ai_response.completion_tokens(),
        ) {
            return Err(Box::new(Error::StorageError(format!(
                "添加消息到会话记录失败：{}, {e}",
                guest.name
            ))));
        }
        tracing::debug!("AI's reply appended");

        Ok(Response {
            content: ai_response.content().to_owned(),
            cost,
        })
    }

    /// 查账单
    fn audit(&self, guest: &core::Guest) -> String {
        // 获取用户会话记录。若会话记录不存在，则创建新记录。
        if let Err(e) = self.storage.get_conversation(guest, self.id) {
            tracing::warn!(
                "获取用户{}会话记录失败：{}。将为此用户创建新记录。",
                guest.name,
                e
            );
            if let Err(e) = self.storage.create_conversation(guest, self.id) {
                tracing::error!("新建用户{}会话记录失败。{}", guest.name, e);
                return format!("内部错误，请稍后再试。{e}");
            }
            tracing::info!("已为用户{}创建会话记录。", guest.name);
        };
        let conversation = self
            .storage
            .get_conversation(guest, self.id)
            .expect("Conversation should be ready");

        format!(
            "当前会话长度为 {}。累计消耗prompt token {}个，completion token {}个，费用{:.3}。",
            conversation.last().unwrap().prompt_tokens
                + conversation.last().unwrap().completion_tokens,
            conversation.iter().fold(0, |acc, x| acc + x.prompt_tokens),
            conversation
                .iter()
                .fold(0, |acc, x| acc + x.completion_tokens),
            conversation.iter().fold(0.0, |acc, x| acc + x.cost)
        )
    }

    // 开始全新会话
    fn new_conversation(
        &self,
        guest: &core::Guest,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(self.storage.create_conversation(guest, self.id)?)
    }
}
