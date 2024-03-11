//! Agent负责协调用户与AI之间的交互过程
use axum::extract::Query;
use axum::http::StatusCode;
use error::Error;
use serde::Deserialize;
use serde::Serialize;
use serde_xml_rs::from_str;
use std::path::Path;

// 企业微信加解密模块
use wecom_crypto::CryptoAgent;

// 企业微信API模块
use wecom_agent::{
    message::{MessageBuilder as WecomMsgBuilder, Text, WecomMessage},
    WecomAgent,
};

// OpenAI API
use super::providers::openai::{MessageRole as OaiMsgRole, OpenAiAgent};

/// 数据库模块
use super::database::{
    Assistant, Conversation as DBConversation, DBAgent, Guest, Message as DBMessage,
};

/// Agent负责协调用户与AI之间的交互过程
pub struct Agent {
    app_token: String,
    ai_agent: OpenAiAgent,
    crypto_agent: CryptoAgent,
    wecom_agent: WecomAgent,
    clerk: DBAgent,
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
            ai_agent: OpenAiAgent::new(oai_endpoint, oai_key),
            crypto_agent: wecom_crypto::CryptoAgent::new(b64encoded_aes_key),
            wecom_agent: WecomAgent::new(corp_id, secret),
            clerk: db_agent.expect("Database should be initialized"),
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

    /// 处理用户发来的请求
    /// 目前应用的管理操作同样使用本接口来实现。故需按照用户角色与内容来协同判断用户请求的意图。
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
        let guest_name = &received_msg.from_user_name;
        let guest: Guest;
        match self.clerk.get_user(guest_name) {
            Err(e) => {
                tracing::error!("获取用户数据失败: {}", e);
                return;
            }
            Ok(None) => {
                tracing::warn!("用户不存在。新建用户: {guest_name}");
                match self.clerk.register(guest_name) {
                    Err(e) => {
                        tracing::error!("新建用户失败: {}", e);
                        return;
                    }
                    Ok(g) => {
                        guest = g;
                    }
                }
            }
            Ok(Some(g)) => guest = g,
        };

        // 是管理员指令吗？
        // 管理员消息由管理员账户(Guest::admin=true)发送，并且内容匹配管理员指令规则(#指令内容)。
        // 此过程出现的任何错误，均需要告知管理员。
        if guest.admin == true && received_msg.content.trim().starts_with("#") {
            let sys_reply =
                match self.handle_admin_msg(received_msg.content.trim().trim_start_matches('#')) {
                    Ok(msg) => msg,
                    Err(e) => format!("处理管理员消息时出错：{}", e),
                };
            let content = Text::new(sys_reply);
            if let Err(e) = self.reply(&received_msg, content).await {
                tracing::error!("回复管理员消息时出错: {}", e);
            }
            return;
        }

        // 处理常规用户消息
        self.handle_guest_msg(&guest, &received_msg).await;
    }

    // 向用户回复一条消息。消息内容content需要满足WecomMessage。
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

    // 处理管理员消息
    // 管理员消息模式："用户名 余额变更量 管理员设定 有效账户"
    fn handle_admin_msg(
        &self,
        msg: &str,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        // Get admin command
        let args: Vec<&str> = msg.split(' ').collect();
        if args.len() == 1 {
            return Err(Box::new(Error::new(
                "使用方式：`::用户名 余额变更量 设定管理员`。例如：::robin 3.14 false".to_string(),
            )));
        }
        if args.len() != 3 {
            return Err(Box::new(Error::new(format!(
                "需要参数3个，实际收到{}个",
                args.len()
            ))));
        }
        let username = args[0];
        let credit_var: f64 = args[1]
            .parse()
            .map_err(|e| Error::new(format!("用户余额解析出错：{}", e)))?;
        let as_admin: bool = args[2]
            .parse()
            .map_err(|e| Error::new(format!("用户管理员属性解析出错：{}", e)))?;

        // Handle the update
        let user = self.clerk.get_user(username)?;
        if user.is_none() {
            return Err(Box::new(Error::new(format!("无法找到用户：{}", username))));
        }
        let updated_user = self
            .clerk
            .update_user(&user.unwrap(), credit_var, as_admin)?;
        Ok(format!(
            "用户{}更新成功。当前余额：{}。管理员：{}",
            username, updated_user.credit, updated_user.admin
        ))
    }

    // 处理常规用户消息
    async fn handle_guest_msg(&self, guest: &Guest, received_msg: &ReceivedMsg) {
        // 用户账户有效？
        if guest.credit < 0.0 {
            tracing::warn!("余额不足。账户{}欠款：{}。", guest.name, guest.credit.abs());
            // 告知用户欠款详情
            let content = Text::new(format!("余额不足。当前账户欠款：{}。", guest.credit.abs()));
            if let Err(e) = self.reply(&received_msg, content).await {
                tracing::error!("发送欠费通知失败：{e}");
            }
            return;
        }

        // 获取当前Assistant
        let agent_id = received_msg.agent_id.parse::<i32>();
        if let Err(e) = agent_id {
            let err_msg = format!("转换AgentID失败：{e}");
            tracing::error!(err_msg);
            let content = Text::new(err_msg);
            if let Err(e) = self.reply(&received_msg, content).await {
                tracing::error!("发送Assistant错误消息时出错: {}", e);
            }
            return;
        }
        let assistant: Assistant;
        match self.clerk.get_assistant_by_agent_id(agent_id.unwrap()) {
            Err(e) => {
                let err_msg = format!("获取Assistant失败：{e}");
                tracing::error!(err_msg);
                let content = Text::new(err_msg);
                if let Err(e) = self.reply(&received_msg, content).await {
                    tracing::error!("发送Assistant错误消息时出错: {}", e);
                }
                return;
            }
            Ok(a) => assistant = a,
        }

        // 账户OK，获取用户会话记录。若会话记录不存在，则创建新记录。
        let conversation: DBConversation;
        match self.clerk.get_active_conversation(&guest) {
            Err(e) => {
                tracing::warn!(
                    "获取用户{}会话记录失败：{}。将为此用户创建新记录。",
                    guest.name,
                    e
                );
                match self.clerk.create_conversation(&guest, &assistant) {
                    Err(e) => {
                        let err_msg = format!("创建用户{}会话记录失败：{}。", guest.name, e);
                        tracing::error!(err_msg);
                        let content = Text::new(err_msg);
                        if let Err(e) = self.reply(&received_msg, content).await {
                            tracing::error!("发送会话错误消息时出错: {}", e);
                        }
                        return;
                    }
                    Ok(c) => conversation = c,
                }
            }
            Ok(c) => conversation = c,
        };

        // 是指令消息吗？指令消息不会计入会话记录。
        let user_msg = received_msg.content.as_str();
        if user_msg.starts_with("#") {
            let reply_content: String;
            match user_msg {
                "#查余额" => reply_content = format!("当前余额：{}", guest.credit),
                "#查token" => match self.clerk.get_messages_by_conversation(&conversation) {
                    Err(e) => {
                        reply_content = format!("获取会话记录失败：{}, {e}", guest.name);
                        tracing::error!(reply_content);
                    }
                    Ok(msgs) => {
                        let mut in_tokens = 0;
                        let mut out_tokens = 0;
                        msgs.iter().for_each(|m| {
                            in_tokens += m.prompt_tokens;
                            out_tokens += m.completion_tokens
                        });
                        reply_content = format!("当前会话累计消耗prompt token {in_tokens}个，completion token {out_tokens}个。");
                    }
                },
                &_ => reply_content = "抱歉，暂不支持当前指令。".to_string(),
            };
            if let Err(e) = self.reply(received_msg, Text::new(reply_content)).await {
                tracing::error!("发送用户指令反馈消息失败：{e}");
            }
            return;
        }

        // 记录用户消息，并与当前会话记录关联
        let content_type = self.clerk.get_content_type_by_name("text");
        if let Err(e) = content_type {
            let err_msg = format!("获取消息内容类型失败：{}, {e}", guest.name);
            tracing::error!(err_msg);
            let content = Text::new(err_msg);
            if let Err(e) = self.reply(&received_msg, content).await {
                tracing::error!("发送消息内容类型错误消息时出错: {}", e);
            }
            return;
        }
        let content_type_text = content_type.as_ref().unwrap();
        if let Err(e) = self.clerk.create_message(
            &conversation,
            &OaiMsgRole::User.into(),
            &received_msg.content,
            content_type_text,
            0.0,
            0,
            0,
        ) {
            let err_msg = format!("新增消息记录失败：{}, {e}", guest.name);
            tracing::error!(err_msg);
            let content = Text::new(err_msg);
            if let Err(e) = self.reply(&received_msg, content).await {
                tracing::error!("发送新增消息错误消息时出错: {}", e);
            }
            return;
        }

        // 获取AI可以处理的会话记录。
        let raw_msgs: Vec<DBMessage>;
        match self.clerk.get_messages_by_conversation(&conversation) {
            Err(e) => {
                let err_msg = format!("获取会话记录失败：{}, {e}", guest.name);
                tracing::error!(err_msg);
                let content = Text::new(err_msg);
                if let Err(e) = self.reply(&received_msg, content).await {
                    tracing::error!("发送获取消息错误消息时出错: {}", e);
                }
                return;
            }
            Ok(r) => raw_msgs = r,
        }

        // 若会话超长，丢弃最早内容。
        let provider = self.clerk.get_provider(assistant.provider_id);
        if let Err(e) = provider {
            let err_msg = format!("获取Provider记录失败：{}, {e}", guest.name);
            tracing::error!(err_msg);
            let content = Text::new(err_msg);
            if let Err(e) = self.reply(&received_msg, content).await {
                tracing::error!("发送获取Provider错误消息时出错: {}", e);
            }
            return;
        }
        let max_tokens = provider.unwrap().max_tokens;
        let mut db_msgs: Vec<&DBMessage> = raw_msgs.iter().collect();
        while db_msgs.last().is_some_and(|m| {
            m.prompt_tokens + m.completion_tokens > (0.9 * max_tokens as f64) as i32
        }) && db_msgs.len() > 1
        {
            db_msgs.remove(1);
        }

        // 交由AI处理
        let response = self.ai_agent.chat(&db_msgs.into()).await;

        // AI接口成功？
        if let Err(e) = &response {
            tracing::error!("获取AI消息失败: {}", e);
            // 告知用户发生内部错误，避免用户徒劳重试或者等待
            let content = Text::new(format!(
                "获取AI回复时发生错误。请等一分钟再试，或者向管理员寻求帮助。"
            ));
            if let Err(e) = self.reply(&received_msg, content).await {
                tracing::error!("发送AI错误通知失败：{e}");
            }
            return;
        }
        let response = response.expect("AI message should be valid");

        // AI返回了有效内容？
        if response.choices().is_empty() {
            tracing::warn!("AI消息为空");
            // 告知用户发生内部错误，避免用户徒劳重试或者等待
            let content = Text::new(format!(
                "AI没有返回有效消息。请等一分钟再试，或者向管理员寻求帮助。"
            ));
            if let Err(e) = self.reply(&received_msg, content).await {
                tracing::error!("发送AI错误通知失败：{e}");
            }
            return;
        }

        // 更新AI回复到会话记录
        let ai_reply: DBMessage;
        match self.clerk.create_message(
            &conversation,
            &OaiMsgRole::Assistant.into(),
            response.choices()[0].message().content(),
            content_type_text,
            response.charge(),
            response.prompt_tokens() as i32,
            response.completion_tokens() as i32,
        ) {
            Err(e) => {
                let err_msg = format!("记录AI消息失败：{}, {e}", guest.name);
                tracing::error!(err_msg);
                let content = Text::new(err_msg);
                if let Err(e) = self.reply(&received_msg, content).await {
                    tracing::error!("发送记录AI消息错误时出错: {}", e);
                }
                return;
            }
            Ok(r) => ai_reply = r,
        }
        tracing::debug!("User {} AI message appended", received_msg.from_user_name);

        // 扣除相应余额
        if let Err(e) = self.clerk.update_user(&guest, response.charge(), false) {
            let err_msg = format!("更新用户账户失败：{}, {e}", guest.name);
            tracing::error!(err_msg);
            let content = Text::new(err_msg);
            if let Err(e) = self.reply(&received_msg, content).await {
                tracing::error!("发送账户更新错误消息时出错: {}", e);
            }
            return;
        }

        // 回复用户最终结果
        let content = Text::new(ai_reply.content);
        if let Err(e) = self.reply(&received_msg, content).await {
            tracing::error!("回复用户消息失败：{e}")
        }
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
