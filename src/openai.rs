use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde::{Deserialize, Serialize};
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

impl Conversation {
    // 新建一组会话记录
    pub fn new(system_msg: Option<&str>) -> Self {
        let mut messages = Vec::<Message>::new();
        let sys_msg = match system_msg {
            Some(msg) => msg,
            None => "You are a helpful assistant.",
        };
        messages.push(Message::new(MessageRole::System, sys_msg.to_owned()));
        Self { messages }
    }

    // 为当前会话记录追加一条消息
    pub fn append(&mut self, msg: &Message) {
        self.messages.push(msg.clone());
    }

    // 清空当前会话记录
    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.messages.clear();
    }
}

// 会话记录中的每一条消息
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Message {
    role: MessageRole,
    content: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum MessageRole {
    #[serde(rename = "system")]
    System,
    #[serde(rename = "user")]
    User,
    #[serde(rename = "assistant")]
    Assistant,
}

impl Message {
    pub fn new(role: MessageRole, content: String) -> Self {
        Self { role, content }
    }

    #[allow(dead_code)]
    pub fn role(&self) -> &MessageRole {
        &self.role
    }

    pub fn content(&self) -> &str {
        &self.content
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
    id: String,
    object: String,
    created: usize,
    model: String,
    usage: ApiUsage,
    pub choices: Vec<ChatResult>,
}

#[derive(Deserialize)]
struct ApiUsage {
    prompt_tokens: usize,
    completion_tokens: usize,
    total_tokens: usize,
}

#[derive(Deserialize)]
pub struct ChatResult {
    pub message: Message,
    finish_reason: String,
    index: usize,
}
