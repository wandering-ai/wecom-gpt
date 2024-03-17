/// OpenAI作为API供应商
use crate::reception::core;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde::{Deserialize, Serialize};
use std::convert::{From, Into};
use std::error::Error;

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
struct Response {
    #[allow(dead_code)]
    id: String,
    #[allow(dead_code)]
    object: String,
    #[allow(dead_code)]
    created: usize,
    #[allow(dead_code)]
    model: String,
    usage: Usage,
    choices: Vec<Choices>,
}

#[derive(Deserialize)]
struct Usage {
    prompt_tokens: usize,
    completion_tokens: usize,
    #[allow(dead_code)]
    total_tokens: usize,
}

#[derive(Deserialize)]
pub struct Choices {
    message: Message,
    #[allow(dead_code)]
    finish_reason: String,
    #[allow(dead_code)]
    index: usize,
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

impl From<&Role> for core::MessageRole {
    fn from(value: &Role) -> Self {
        match value {
            Role::Assistant => core::MessageRole::Assistant,
            Role::System => core::MessageRole::System,
            Role::User => core::MessageRole::User,
            _ => core::MessageRole::Supplementary,
        }
    }
}

impl From<Role> for core::MessageRole {
    fn from(value: Role) -> Self {
        value.into()
    }
}

impl From<&core::MessageRole> for Role {
    fn from(value: &core::MessageRole) -> Self {
        match value {
            core::MessageRole::Assistant => Role::Assistant,
            core::MessageRole::System => Role::System,
            core::MessageRole::User => Role::User,
            core::MessageRole::Supplementary => Role::Assistant, // 暂定，对用户影响最小
        }
    }
}

impl From<core::MessageRole> for Role {
    fn from(value: core::MessageRole) -> Self {
        value.into()
    }
}

// 会话记录中的每一条消息
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Message {
    role: Role,
    content: String,
}

// 消息类型转换
impl From<&core::Message> for Message {
    fn from(value: &core::Message) -> Self {
        Self {
            role: value.role.clone().into(),
            content: value.content.clone(),
        }
    }
}

impl From<core::Message> for Message {
    fn from(value: core::Message) -> Self {
        value.into()
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
#[derive(Serialize)]
pub struct Conversation {
    messages: Vec<Message>, // 注意名字要与Json格式匹配
}

impl From<&core::Conversation> for Conversation {
    fn from(value: &core::Conversation) -> Self {
        let mut messages = Vec::<Message>::new();
        for msg in value.content.iter() {
            messages.push(Message::from(msg));
        }

        Self { messages }
    }
}

// AI回复结构体，包含成本信息
struct OaiResponse {
    pub prompt_token_price: f64,
    pub completion_token_price: f64,
    pub response: Response,
}

impl core::ChatResponse for OaiResponse {
    fn content(&self) -> &str {
        match self.response.choices.get(0) {
            Some(r) => &r.message.content,
            None => "",
        }
    }

    fn role(&self) -> core::MessageRole {
        match self.response.choices.get(0) {
            Some(r) => (&r.message.role).into(),
            None => core::MessageRole::Supplementary,
        }
    }

    fn cost(&self) -> f64 {
        ((self.prompt_token_price * self.response.usage.prompt_tokens as f64)
            + (self.completion_token_price * self.response.usage.completion_tokens as f64))
            / 1000.0
    }

    fn tokens(&self) -> usize {
        self.response.usage.total_tokens
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

impl core::Chat for Agent {
    // 根据会话内容，返回最新消息。
    async fn chat(
        &self,
        conversation: &core::Conversation,
    ) -> Result<impl core::ChatResponse, Box<dyn Error + Send + Sync>> {
        let mut conv = conversation.clone();

        // System Message完整？
        if conv
            .content
            .first()
            .is_some_and(|m| m.role != core::MessageRole::System)
        {
            conv.content.insert(
                0,
                core::Message {
                    content: DEFAULT_SYSTEM_MSG.to_owned(),
                    role: core::MessageRole::System,
                    cost: 0.0,
                    tokens: 0,
                },
            );
        }

        // 会话超长？移除第一条非系统消息直到满足要求。注意长度不要越界。
        if let Some(latest) = conv.content.last() {
            let mut current_tokens = latest.tokens;
            while current_tokens > (self.deployment.max_tokens as f64 * 0.9) as usize
                && conv.content.len() >= 2
            {
                current_tokens -= conv.content.get(1).unwrap().tokens;
                conv.content.remove(1);
            }
        }

        // 交由AI处理
        let oai_conv: Conversation = (&conv).into();
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
            .json(&oai_conv)
            .headers(header)
            .send()
            .await?
            .json::<Response>()
            .await?;

        Ok(OaiResponse {
            prompt_token_price: self.deployment.prompt_token_price,
            completion_token_price: self.deployment.completion_token_price,
            response,
        })
    }
}
