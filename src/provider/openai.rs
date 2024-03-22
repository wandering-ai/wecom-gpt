//! OpenAI作为API供应商
use crate::storage::model;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde::{Deserialize, Serialize};
use std::convert::{From, TryFrom};
use std::fmt;
use std::string::ToString;

// Custom Error
#[derive(Debug, Clone)]
pub struct Error(String);

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
impl std::error::Error for Error {}

const DEFAULT_SYSTEM_MSG: &str = "You are a helpful assistant.";

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
pub struct Response {
    #[allow(dead_code)]
    id: String,
    #[allow(dead_code)]
    object: String,
    #[allow(dead_code)]
    created: u64,
    #[allow(dead_code)]
    model: String,
    pub usage: Usage,
    pub choices: Vec<Choice>,
}

impl Response {
    pub fn content(&self) -> &str {
        tracing::debug!("Returning content..");
        match self.choices.first() {
            Some(c) => &c.message.content,
            None => "",
        }
    }

    pub fn role(&self) -> Role {
        tracing::debug!("Returning message role..");
        match self.choices.first() {
            Some(c) => Role::try_from(c.message.role.as_str()).unwrap(),
            None => Role::System, // Never happens
        }
    }

    pub fn prompt_tokens(&self) -> u64 {
        tracing::debug!("Returning cost..");
        self.usage.prompt_tokens
    }

    pub fn completion_tokens(&self) -> u64 {
        tracing::debug!("Returning cost..");
        self.usage.completion_tokens
    }
}

#[derive(Deserialize)]
pub struct Usage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    #[allow(dead_code)]
    pub total_tokens: u64,
}

#[derive(Deserialize)]
pub struct Choice {
    pub message: Message,
    #[allow(dead_code)]
    finish_reason: String,
    #[allow(dead_code)]
    index: u64,
}

// 消息角色枚举。来自OpenAI的定义
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum Role {
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

impl Role {
    pub fn to_id(&self) -> i32 {
        match self {
            Role::System => 1,
            Role::User => 2,
            Role::Assistant => 3,
            Role::Tool => 4,
            Role::Function => 5,
        }
    }
}

impl TryFrom<&str> for Role {
    type Error = &'static str;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "system" => Ok(Role::System),
            "user" => Ok(Role::User),
            "assistant" => Ok(Role::Assistant),
            "tool" => Ok(Role::Tool),
            "function" => Ok(Role::Function),
            &_ => Err("Unknown chat role"),
        }
    }
}

impl TryFrom<i32> for Role {
    type Error = &'static str;
    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Role::System),
            2 => Ok(Role::User),
            3 => Ok(Role::Assistant),
            4 => Ok(Role::Tool),
            5 => Ok(Role::Function),
            _ => Err("Unknown chat role"),
        }
    }
}

impl ToString for Role {
    fn to_string(&self) -> String {
        match self {
            Role::System => "system".to_string(),
            Role::User => "user".to_string(),
            Role::Assistant => "assistant".to_string(),
            Role::Tool => "tool".to_string(),
            Role::Function => "function".to_string(),
        }
    }
}

// 会话记录中的每一条消息
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Message {
    pub role: String,
    pub content: String,
}

impl From<&model::Message> for Message {
    fn from(value: &model::Message) -> Self {
        Self {
            role: Role::try_from(value.message_type)
                .expect("Role id should be a valid int")
                .to_string(),
            content: value.content.clone(),
        }
    }
}

// 会话记录
// 发送给OpenAI的会话需要满足本格式要求
//   "messages": [
//     {"role": "system",
//       "content": "You are a helpful assistant."},
//     {"role": "user",
//       "content": "Does Azure OpenAI support customer managed keys?"},
//     {"role": "assistant",
//       "content": "Yes, customer managed keys are supported by Azure OpenAI."},
//     {"role": "user",
//       "content": "Do other Azure AI services support this too?"}
//   ]
#[derive(Serialize, Clone)]
pub struct Conversation {
    pub messages: Vec<Message>, // 注意名字要与Json格式匹配
}

// AI供应商服务所需要的参数
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub id: u64,
    pub name: String,
    pub endpoint: String,
    pub api_key: String,
    pub max_tokens: u64,
    pub prompt_token_price: f64,
    pub completion_token_price: f64,
}

#[derive(Debug, Clone)]
pub struct Agent {
    config: Config,
    client: reqwest::Client,
}

impl Agent {
    pub fn new(config: &Config) -> Self {
        Self {
            config: config.clone(),
            client: reqwest::Client::new(),
        }
    }

    /// Token长度限制
    pub fn max_tokens(&self) -> u64 {
        self.config.max_tokens
    }

    // 根据会话内容，返回最新消息。
    pub async fn process(
        &self,
        conversation: &Conversation,
        prompt: Option<&str>,
    ) -> Result<Response, Error> {
        let mut conv = conversation.clone();

        // 交由AI处理
        tracing::debug!("Ask AI for response..");
        let header = {
            let mut headers = HeaderMap::new();
            headers.insert(
                HeaderName::from_static("api-key"),
                HeaderValue::from_str(&self.config.api_key).expect("API key should be parsed"),
            );
            headers
        };
        let response = self
            .client
            .post(&self.config.endpoint)
            .json(&conv)
            .headers(header)
            .send()
            .await
            .map_err(|e| Error(format!("发送AI请求失败。{e}")))?
            .json::<Response>()
            .await
            .map_err(|e| Error(format!("接收AI返回失败。{e}")))?;
        Ok(response)
    }

    /// 计算价值消耗
    pub fn cost(&self, response: &Response) -> f64 {
        (self.config.prompt_token_price * response.prompt_tokens() as f64
            + self.config.completion_token_price * response.completion_tokens() as f64)
            / 1000.0
    }
}
