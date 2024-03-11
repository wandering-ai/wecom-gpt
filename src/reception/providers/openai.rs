use crate::reception::database;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde::{Deserialize, Serialize};
use std::convert::{From, Into};
use std::error::Error as StdError;

#[derive(Debug, Clone)]
pub struct OpenAiAgent {
    endpoint: String,
    api_key: String,
    client: reqwest::Client,
}

impl OpenAiAgent {
    pub fn new(endpoint: &str, api_key: &str) -> Self {
        Self {
            endpoint: endpoint.to_string(),
            api_key: api_key.to_string(),
            client: reqwest::Client::new(),
        }
    }

    // 请求Chat API
    pub async fn chat(
        &self,
        conversation: &Conversation,
    ) -> Result<ChatResponse, Box<dyn StdError + Send + Sync>> {
        let header = {
            let mut headers = HeaderMap::new();
            headers.insert(
                HeaderName::from_static("api-key"),
                HeaderValue::from_str(&self.api_key).expect("API key should be valid"),
            );
            headers
        };
        let response = self
            .client
            .post(&self.endpoint)
            .json(&conversation)
            .headers(header)
            .send()
            .await?
            .json::<ChatResponse>()
            .await?;
        Ok(response)
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
pub struct Conversation {
    messages: Vec<Message>,
}

impl From<Vec<database::Message>> for Conversation {
    fn from(value: Vec<database::Message>) -> Self {
        value.into()
    }
}

impl From<Vec<&database::Message>> for Conversation {
    fn from(value: Vec<&database::Message>) -> Self {
        let mut messages = Vec::<Message>::new();

        // 首条消息应当为系统消息
        let mut msg_iter = value.iter();
        let sys_msg = match value.len() {
            n if n > 0 => &msg_iter.next().unwrap().content,
            _ => "You are a helpful assistant.",
        };
        messages.push(Message::new(MessageRole::System, sys_msg.to_owned()));

        // 追加剩余消息
        while let Some(msg) = msg_iter.next() {
            messages.push(Message::from(*msg));
        }

        Self { messages }
    }
}

// 会话记录中的每一条消息
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Message {
    role: MessageRole,
    content: String,
}

impl Message {
    pub fn new(role: MessageRole, content: String) -> Self {
        Self { role, content }
    }

    pub fn content(&self) -> &str {
        &self.content
    }
}

// 数据库消息转换为当前模块消息
impl From<database::Message> for Message {
    fn from(value: database::Message) -> Self {
        value.into()
    }
}

impl From<&database::Message> for Message {
    fn from(value: &database::Message) -> Self {
        Self {
            role: match value.message_type {
                1 => MessageRole::System,
                2 => MessageRole::User,
                3 => MessageRole::Assistant,
                i32::MIN..=0_i32 | 4_i32..=i32::MAX => MessageRole::User,
            },
            content: value.content.clone(),
        }
    }
}

// 消息角色枚举
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum MessageRole {
    #[serde(rename = "system")]
    System,
    #[serde(rename = "user")]
    User,
    #[serde(rename = "assistant")]
    Assistant,
}

// 消息角色转换为数据库角色
impl Into<database::MessageType> for MessageRole {
    fn into(self) -> database::MessageType {
        match self {
            MessageRole::System => database::MessageType {
                id: 1,
                name: "system".to_string(),
            },
            MessageRole::User => database::MessageType {
                id: 2,
                name: "user".to_string(),
            },
            MessageRole::Assistant => database::MessageType {
                id: 3,
                name: "assistant".to_string(),
            },
        }
    }
}

// 数据库角色转换为消息角色
impl From<database::MessageType> for MessageRole {
    fn from(value: database::MessageType) -> Self {
        value.into()
    }
}
impl From<&database::MessageType> for MessageRole {
    fn from(value: &database::MessageType) -> Self {
        match value.name.as_str() {
            "system" => Self::System,
            "user" => Self::User,
            "assistant" => Self::Assistant,
            &_ => Self::User,
        }
    }
}

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
pub struct ChatResponse {
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

impl ChatResponse {
    pub fn choices(&self) -> &Vec<ChatResult> {
        &self.choices
    }

    pub fn charge(&self) -> f64 {
        (self.usage.prompt_tokens as f64 * 0.06 + self.usage.completion_tokens as f64 * 0.12)
            / 1000.0
    }

    pub fn prompt_tokens(&self) -> usize {
        self.usage.prompt_tokens
    }

    pub fn completion_tokens(&self) -> usize {
        self.usage.completion_tokens
    }
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
    message: Message,
    #[allow(dead_code)]
    finish_reason: String,
    #[allow(dead_code)]
    index: usize,
}

impl ChatResult {
    pub fn message(&self) -> &Message {
        &self.message
    }
}
