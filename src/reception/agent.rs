use super::guest::{Guest, Message, MessageRole};
use std::collections::HashMap;
use std::error::Error as StdError;

// OpenAI API
use super::providers::openai;

#[derive(Clone)]
// Agent负责协调用户与AI之间的交互过程
pub struct Agent {
    guest_list: HashMap<String, Guest>,
    ai_agent: openai::OpenAiAgent,
}

impl Agent {
    // 新建一个Agent
    pub fn new(endpoint: &str, api_key: &str) -> Self {
        let oai_agent = openai::OpenAiAgent::new(endpoint, api_key);
        let guest_list = HashMap::<String, Guest>::new();
        Self {
            guest_list,
            ai_agent: oai_agent,
        }
    }

    // 处理用户的消息
    pub async fn handle_user_message(
        &mut self,
        user_name: &str,
        msg: &str,
    ) -> Result<String, Box<dyn StdError + Send + Sync>> {
        // Safety first
        let guest = self
            .guest_list
            .entry(user_name.to_owned())
            .or_insert(Guest::new(user_name, 1.0)?);
        if guest.credit() < 0.0 {
            return Err(Box::new(error::Error::new("余额不足。".to_string())));
        }

        // Construct conversation for AI to handle
        guest.append_message(Message::new(msg, &None, 0.0, MessageRole::User)?);
        let conversation = guest.get_conversation();

        // Handle the message
        let response = self.ai_agent.chat(&conversation.into()).await?;

        // Post process
        let reply = response.choices[0].message.content();
        guest.append_message(Message::new(
            reply,
            &None,
            response.charge(),
            MessageRole::Assistant,
        )?);

        Ok(reply.into())
    }
}

pub mod error {
    use std::error::Error as StdError;
    use std::fmt;

    #[derive(Debug, Clone)]
    pub struct Error(String);

    impl Error {
        pub fn new(text: String) -> Self {
            Self(text)
        }
    }

    impl fmt::Display for Error {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "{}", self.0)
        }
    }

    impl StdError for Error {}
}
