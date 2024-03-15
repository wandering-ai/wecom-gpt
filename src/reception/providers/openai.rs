/// OpenAI作为API供应商
use crate::reception::core::{AIConversation, AIMessage, AIProvider, MessageRole};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde::{Deserialize, Serialize};
use std::convert::{From, Into};
use std::error::Error;

// Chat请求返回结果
// 示例
// {
//     "id":"chatcmpl-6v7mkQj980V1yBec6ETrKPRqFjNw9",
//     "object":"chat.completion",
//     "created":1679072642,
//     "model":"gpt-35-turbo",
//     "usage":{
//        "prompt_tokens":58,
//        "completion_tokens":68,
//        "total_tokens":126
//     },
//     "choices":[
//        {
//           "message":{
//              "role":"assistant",
//              "content":"Yes, other Azure AI services also support customer managed keys. Azure AI services offer multiple options for customers to manage keys, such as using Azure Key Vault, customer-managed keys in Azure Key Vault or customer-managed keys through Azure Storage service. This helps customers ensure that their data is secure and access to their services is controlled."
//           },
//           "finish_reason":"stop",
//           "index":0
//        }
//     ]
// }
#[derive(Deserialize)]
struct ChatResponse {
    #[allow(dead_code)]
    id: String,
    #[allow(dead_code)]
    object: String,
    #[allow(dead_code)]
    created: usize,
    #[allow(dead_code)]
    model: String,
    usage: ApiUsage,
    choices: Vec<ChatResult>,
}

#[derive(Deserialize)]
struct ApiUsage {
    prompt_tokens: usize,
    completion_tokens: usize,
    #[allow(dead_code)]
    total_tokens: usize,
}

#[derive(Deserialize)]
pub struct ChatResult {
    message: ChatMessage,
    #[allow(dead_code)]
    finish_reason: String,
    #[allow(dead_code)]
    index: usize,
}

// 会话记录中的每一条消息
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct ChatMessage {
    role: ChatRole,
    content: String,
}

impl ChatMessage {
    pub fn new(role: ChatRole, content: String) -> Self {
        Self { role, content }
    }
}

impl<T> From<T> for ChatMessage
where
    T: AIMessage,
{
    fn from(value: T) -> Self {
        Self {
            role: value.role().into(),
            content: value.content().to_string(),
        }
    }
}

// 消息角色枚举。来自OpenAI的定义
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum ChatRole {
    #[serde(rename = "system")]
    System,
    #[serde(rename = "user")]
    User,
    #[serde(rename = "assistant")]
    Assistant,
    #[serde(rename = "tool")]
    Tool,
    #[serde(rename = "function")]
    Function,
}

impl From<ChatRole> for MessageRole {
    fn from(value: ChatRole) -> Self {
        match value {
            ChatRole::Assistant => MessageRole::Assistant,
            ChatRole::System => MessageRole::System,
            ChatRole::User => MessageRole::User,
            _ => MessageRole::Supplementary,
        }
    }
}

impl From<MessageRole> for ChatRole {
    fn from(value: MessageRole) -> Self {
        match value {
            MessageRole::Assistant => ChatRole::Assistant,
            MessageRole::System => ChatRole::System,
            MessageRole::User => ChatRole::User,
            MessageRole::Supplementary => ChatRole::Tool,
        }
    }
}

// 消息结构体，包含成本模型
struct ChargeableMsg {
    pub prompt_token_price: f64,
    pub completion_token_price: f64,
    pub response: ChatResponse,
}

impl AIMessage for ChargeableMsg {
    fn content(&self) -> &str {
        match self.response.choices.get(0) {
            Some(r) => &r.message.content,
            None => "",
        }
    }

    fn role(&self) -> MessageRole {
        match self.response.choices.get(0) {
            Some(r) => r.message.role.into(),
            None => MessageRole::Supplementary,
        }
    }

    fn cost(&self) -> f64 {
        ((self.prompt_token_price * self.response.usage.prompt_tokens as f64)
            + (self.completion_token_price * self.response.usage.completion_tokens as f64))
            / 1000.0
    }
}

// 会话记录
// 示例
// {
//     "messages":[
//        {
//           "role":"system",
//           "content":"You are a helpful assistant."
//        },
//        {
//           "role":"user",
//           "content":"Does Azure OpenAI support customer managed keys?"
//        },
//        {
//           "role":"assistant",
//           "content":"Yes, customer managed keys are supported by Azure OpenAI."
//        },
//        {
//           "role":"user",
//           "content":"Do other Azure AI services support this too?"
//        }
//     ]
// }
#[derive(Serialize)]
pub struct ChatConversation {
    messages: Vec<ChatMessage>,
}

impl<T> From<T> for ChatConversation
where
    T: AIConversation,
{
    fn from(value: T) -> Self {
        let mut messages = Vec::<ChatMessage>::new();

        // 首条消息应当为系统消息
        let mut msg_iter = value.messages().iter();
        let sys_msg = match value.messages().len() {
            n if n > 0 => &msg_iter.next().unwrap().content(),
            _ => "You are a helpful assistant.",
        };
        messages.push(ChatMessage::new(ChatRole::System, sys_msg.to_owned()));

        // 追加剩余消息
        for msg in msg_iter {
            messages.push(ChatMessage::from(**msg));
        }

        Self { messages }
    }
}

// OpenAI模型部署方案
#[derive(Debug, Clone)]
pub struct Deployment {
    pub endpoint: String,
    pub api_key: String,
    pub prompt_token_price: f64,
    pub completion_token_price: f64,
    pub max_tokens: u64,
}

#[derive(Debug, Clone)]
pub struct Agent {
    deployment: Deployment,
    client: reqwest::Client,
}

impl Agent {
    pub fn new(deployment: Deployment) -> Self {
        Self {
            deployment,
            client: reqwest::Client::new(),
        }
    }
}

impl AIProvider for Agent {
    async fn chat<T>(&self, conversation: T) -> Result<impl AIMessage, Box<dyn Error + Send + Sync>>
    where
        T: Into<ChatConversation> + Serialize,
    {
        let header = {
            let mut headers = HeaderMap::new();
            headers.insert(
                HeaderName::from_static("api-key"),
                HeaderValue::from_str(&self.deployment.api_key).expect("API key should be parsed"),
            );
            headers
        };

        let response = self
            .client
            .post(&self.deployment.endpoint)
            .json(&conversation)
            .headers(header)
            .send()
            .await?
            .json::<ChatResponse>()
            .await?;

        Ok(ChargeableMsg {
            prompt_token_price: self.deployment.prompt_token_price,
            completion_token_price: self.deployment.completion_token_price,
            response,
        })
    }
}
