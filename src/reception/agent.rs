//! Agent负责协调用户与AI之间的交互过程
use axum::extract::Query;
use axum::http::StatusCode;
use error::Error;
use serde::Deserialize;
use serde::Serialize;
use serde_xml_rs::from_str;
use std::collections::HashMap;
use std::path::Path;
use tokio::sync::RwLock;

// 企业微信加解密模块
use wecom_crypto::CryptoAgent;

// 企业微信API模块
use wecom_agent::{
    message::{MessageBuilder as WecomMsgBuilder, Text, WecomMessage},
    WecomAgent,
};

// OpenAI API
use super::providers::openai::{ChatResponse, OpenAiAgent};

// 客户抽象
use super::guest::{Guest, Message, MessageRole};

/// 数据库模块
use super::database::DBAgent;

/// Agent负责协调用户与AI之间的交互过程
pub struct Agent {
    app_token: String,
    guest_list: RwLock<HashMap<String, Guest>>,
    ai_agent: OpenAiAgent,
    crypto_agent: CryptoAgent,
    wecom_agent: WecomAgent,
    db_agent: DBAgent,
}

impl Agent {
    /// 新建一个应用Agent
    pub fn new(
        app_token: &str,
        b64encoded_aes_key: &str,
        corp_id: &str,
        secret: &str,
        oai_endpoint: &str,
        oai_key: &str,
        db_path: &Path,
    ) -> Result<Self, Error> {
        let db_agent = DBAgent::new(db_path.to_str().expect("Database path should be valid"));
        if let Err(e) = db_agent {
            return Err(Error::new(format!("数据库初始化失败：{}", e)));
        }

        Ok(Self {
            app_token: String::from(app_token),
            guest_list: RwLock::new(HashMap::<String, Guest>::new()),
            ai_agent: OpenAiAgent::new(oai_endpoint, oai_key),
            crypto_agent: wecom_crypto::CryptoAgent::new(b64encoded_aes_key),
            wecom_agent: WecomAgent::new(corp_id, secret),
            db_agent: db_agent.expect("Database should be initialized"),
        })
    }

    /// 配合企业微信，验证服务器地址的有效性。
    pub fn verify_url(&self, params: Query<UrlVerifyParams>) -> Result<String, StatusCode> {
        // Is this request safe?
        if wecom_crypto::generate_signature(vec![
            &params.timestamp,
            &params.nonce,
            &self.app_token,
            &params.echostr,
        ]) != params.msg_signature
        {
            tracing::error!("Error! Code: {}", StatusCode::BAD_REQUEST);
            return Err(StatusCode::BAD_REQUEST);
        }

        // Give the server what it expects.
        match self.crypto_agent.decrypt(&params.echostr) {
            Ok(t) => Ok(t.text),
            Err(e) => {
                tracing::error!("Error in decrypting: {}", e);
                Err(StatusCode::INTERNAL_SERVER_ERROR)
            }
        }
    }

    // 处理用户发来的请求
    pub async fn handle_user_request(&self, params: Query<UserMsgParams>, body: String) {
        // Extract the request body
        let body: RequestBody = from_str(&body).unwrap();

        // Is this request safe?
        if wecom_crypto::generate_signature(vec![
            &params.timestamp,
            &params.nonce,
            &self.app_token,
            &body.encrypted_str,
        ]) != params.msg_signature
        {
            tracing::error!("签名校验失败。数据可能被篡改。");
            return;
        }

        // Decrypt the user message
        let decrypt_result = self.crypto_agent.decrypt(&body.encrypted_str);
        if let Err(e) = &decrypt_result {
            tracing::error!("解密用户数据失败: {}", e);
            return;
        }

        // Parse the xml document
        let xml_doc = from_str::<ReceivedMsg>(&decrypt_result.unwrap().text);
        if let Err(e) = &xml_doc {
            tracing::error!("解析xml失败: {}", e);
            return;
        }
        let received_msg: ReceivedMsg = xml_doc.expect("XML document should be valid.");

        // 谁发送的消息？
        let user_ready = {
            let guest_list = self.guest_list.read().await;
            guest_list.contains_key(&received_msg.from_user_name)
        };
        tracing::debug!("User {} ready: {}", received_msg.from_user_name, user_ready);
        if !user_ready {
            let mut guest_list = self.guest_list.write().await;
            guest_list.insert(
                received_msg.from_user_name.clone(),
                Guest::new(&received_msg.from_user_name, 1.0)
                    .expect("Create new user should not fail."),
            );
            tracing::debug!("User {} created", received_msg.from_user_name);
        }

        // 发送账户有效？
        {
            let guest_list = self.guest_list.read().await;
            let guest = guest_list
                .get(&received_msg.from_user_name)
                .expect("User should exist");

            if guest.credit() < 0.0 {
                tracing::warn!(
                    "余额不足。账户{}欠款：{}。",
                    guest.name(),
                    guest.credit().abs()
                );

                // 告知用户欠款详情
                let content = Text::new(format!(
                    "余额不足。当前账户欠款：{}。",
                    guest.credit().abs()
                ));
                if let Err(e) = self.reply(&received_msg, content).await {
                    tracing::error!("发送欠费通知失败：{e}");
                }
                return;
            }
        }

        // 账户OK，更新用户消息内容
        {
            let mut guest_list = self.guest_list.write().await;
            let guest = guest_list
                .get_mut(&received_msg.from_user_name)
                .expect("User should exist");
            guest.append_message(
                Message::new(&received_msg.content, &None, 0.0, MessageRole::User)
                    .expect("Create new message should not fail"),
            );
            tracing::debug!(
                "User {} updated with user message",
                received_msg.from_user_name
            );
        }

        // 获取AI回复
        let response: Result<ChatResponse, Box<dyn std::error::Error + Send + Sync>>;
        {
            let guest_list = self.guest_list.read().await;
            let guest = guest_list
                .get(&received_msg.from_user_name)
                .expect("User should exist");

            // 获取AI可以处理的会话记录
            let conversation = guest.get_conversation();

            // 交由AI处理
            response = self.ai_agent.chat(&conversation.into()).await;
            tracing::debug!("User {} AI message got", received_msg.from_user_name);
        }

        // AI接口成功？
        if let Err(e) = &response {
            tracing::error!("获取AI消息失败: {}", e);
            // 告知用户发生内部错误，避免用户徒劳重试或者等待
            let content = Text::new(format!(
                "发生内部错误。请等一分钟再试，或者向管理员寻求帮助。"
            ));
            if let Err(e) = self.reply(&received_msg, content).await {
                tracing::error!("发送错误通知失败：{e}");
            }
            return;
        }
        let response = response.expect("AI message should be valid");

        // AI返回了有效内容？
        if response.choices.is_empty() {
            tracing::warn!("AI消息为空");
            // 告知用户发生内部错误，避免用户徒劳重试或者等待
            let content = Text::new(format!(
                "发生内部错误。请等一分钟再试，或者向管理员寻求帮助。"
            ));
            if let Err(e) = self.reply(&received_msg, content).await {
                tracing::error!("发送错误通知失败：{e}");
            }
            return;
        }

        // 更新AI回复到会话记录
        {
            let mut guest_list = self.guest_list.write().await;
            let guest = guest_list
                .get_mut(&received_msg.from_user_name)
                .expect("User should be existed");
            guest.append_message(
                Message::new(
                    response.choices[0].message.content(),
                    &None,
                    response.charge(),
                    MessageRole::Assistant,
                )
                .expect("Create new message should not fail"),
            );
            tracing::debug!("User {} AI message appended", received_msg.from_user_name);
        }

        // 回复用户最终结果
        let content = Text::new(response.choices[0].message.content().to_owned());
        let result = self.reply(&received_msg, content).await;
        match result {
            Err(e) => tracing::error!("回复用户消息失败：{e}"),
            Ok(_) => tracing::debug!("User {} replied", received_msg.from_user_name),
        }
    }

    // 向用户回复一条消息
    async fn reply<T>(
        &self,
        received_msg: &ReceivedMsg,
        content: T,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
    where
        T: Serialize + WecomMessage,
    {
        let msg = WecomMsgBuilder::default()
            .to_users(vec![&received_msg.from_user_name])
            .from_agent(
                received_msg
                    .agent_id
                    .parse::<usize>()
                    .expect("Agent ID should be usize"),
            )
            .build(content)
            .expect("Massage should be built");

        // 发送该消息
        tracing::debug!("Sending message to {} ...", received_msg.from_user_name);
        let response = self.wecom_agent.send(msg).await;
        if let Err(e) = response {
            tracing::debug!("Error sending msg: {e}");
            return Err(Box::new(Error::new(format!("Error sending msg: {e}"))));
        }
        let response = response.expect("Response should be valid.");

        // 发送成功，但是服务器返回错误。
        if response.is_error() {
            tracing::debug!(
                "Wecom API error: {} {}",
                response.error_code(),
                response.error_msg()
            );
            return Err(Box::new(Error::new(format!(
                "Error sending msg: {}, {}",
                response.error_code(),
                response.error_msg()
            ))));
        }
        Ok(())
    }
}

/// 服务器可用性验证请求涉及到的参数
#[derive(Deserialize)]
pub struct UrlVerifyParams {
    msg_signature: String,
    timestamp: String,
    nonce: String,
    echostr: String,
}

/// 用户主动发送来的请求涉及到的参数
#[derive(Deserialize)]
pub struct UserMsgParams {
    msg_signature: String,
    nonce: String,
    timestamp: String,
}

// 请求Body结构体
// <xml>
//   <ToUserName><![CDATA[toUser]]></ToUserName>
//   <AgentID><![CDATA[toAgentID]]></AgentID>
//   <Encrypt><![CDATA[msg_encrypt]]></Encrypt>
// </xml>
#[derive(Debug, Deserialize, PartialEq)]
struct RequestBody {
    #[serde(rename = "ToUserName")]
    to_user_name: String,
    #[serde(rename = "AgentID")]
    agent_id: String,
    #[serde(rename = "Encrypt")]
    encrypted_str: String,
}

// 存储用户所发送消息的结构体
// <xml>
//   <ToUserName><![CDATA[ww637951f75e40d82b]]></ToUserName>
//   <FromUserName><![CDATA[YinGuoBing]]></FromUserName>
//   <CreateTime>1708218294</CreateTime>
//   <MsgType><![CDATA[text]]></MsgType>
//   <Content><![CDATA[[呲牙]]]></Content>
//   <MsgId>7336741709953816625</MsgId>
//   <AgentID>1000002</AgentID>
// </xml>
#[derive(Debug, Deserialize, PartialEq)]
struct ReceivedMsg {
    #[serde(rename = "ToUserName")]
    to_user_name: String,
    #[serde(rename = "FromUserName")]
    from_user_name: String,
    #[serde(rename = "CreateTime")]
    create_time: usize,
    #[serde(rename = "MsgType")]
    msg_type: String,
    #[serde(rename = "Content")]
    content: String,
    #[serde(rename = "MsgId")]
    msg_id: String,
    #[serde(rename = "AgentID")]
    agent_id: String,
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
