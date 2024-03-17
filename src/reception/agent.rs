//! Agent负责协调用户与AI之间的交互过程
use axum::extract::Query;
use axum::http::StatusCode;
use serde::Deserialize;
use serde::Serialize;
use serde_xml_rs::from_str;
use std::fmt;

// 企业微信加解密模块
use wecom_crypto::CryptoAgent;

// 企业微信API模块
use wecom_agent::{
    message::{MessageBuilder as WecomMsgBuilder, Text as WecomText, WecomMessage},
    WecomAgent,
};

// OpenAI API
use super::providers::openai::{Agent as OaiAgent, Deployment as OaiDeploy};

// 核心概念
use super::core::{Chat, ChatResponse, Conversation, Guest, Message, MessageRole, PersistStore};

// 数据库模块
use super::database::Agent as DBAgent;

/// Agent负责协调用户与AI之间的交互过程
pub struct Agent {
    app_token: String,
    ai_agent: OaiAgent,
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
        provider_id: i32,
        oai_key: &str,
        db_path: &str,
    ) -> Result<Self, Error> {
        // 首先初始化数据库
        let db_agent = match DBAgent::new(db_path) {
            Err(e) => return Err(Error::new(format!("数据库初始化失败：{}", e))),
            Ok(a) => a,
        };

        // 从数据库中读取AI provider信息
        let ai_provider = db_agent
            .get_provider(provider_id)
            .map_err(|e| Error(e.to_string()))?;
        let oai_deploy = OaiDeploy {
            endpoint: ai_provider.endpoint,
            api_key: oai_key.to_owned(),
            prompt_token_price: ai_provider.prompt_token_price,
            completion_token_price: ai_provider.completion_token_price,
            max_tokens: ai_provider.max_tokens as u64,
        };

        Ok(Self {
            app_token: String::from(app_token),
            ai_agent: OaiAgent::new(oai_deploy),
            crypto_agent: wecom_crypto::CryptoAgent::new(b64encoded_aes_key),
            wecom_agent: WecomAgent::new(corp_id, secret),
            clerk: db_agent,
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
        let guest_name = received_msg.from_user_name.as_str();
        let guest = match self.clerk.get_user(guest_name) {
            Err(_) => {
                tracing::warn!("用户不存在。将新建用户: {guest_name}");
                let guest = Guest {
                    name: guest_name.to_owned(),
                    credit: 0.0,
                    admin: false,
                };
                match self.clerk.create_user(&guest) {
                    Err(e) => {
                        tracing::error!("新建用户失败: {e}");
                        return;
                    }
                    Ok(_) => {
                        tracing::info!("新建用户成功: {guest_name}");
                        guest
                    }
                }
            }
            Ok(g) => g,
        };

        // 是管理员指令吗？
        // 管理员消息由管理员账户(Guest::admin=true)发送，并且内容匹配管理员指令规则。
        // 管理员指令格式：$$指令内容$$
        if guest.admin
            && received_msg.content.trim().starts_with("$$")
            && received_msg.content.trim().ends_with("$$")
        {
            self.handle_admin_msg(&received_msg).await;
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

    // 回复消息。当遇到错误时记录。
    async fn reply_n_log(&self, msg: &str, received_msg: &ReceivedMsg) {
        let content = WecomText::new(msg.to_owned());
        tracing::error!(msg);
        if let Err(e) = self.reply(received_msg, content).await {
            tracing::error!("回复消息时出错: {}", e);
        }
    }

    // 处理管理员消息
    // 管理员指令内容："用户名 操作名 操作内容"。例如"小白 充值 3.5"。
    // 此过程出现的任何错误，均需要告知管理员。
    async fn handle_admin_msg(&self, received_msg: &ReceivedMsg) {
        // Get admin command
        let msg = received_msg.content.trim_matches('$');
        let args: Vec<&str> = msg.split(' ').collect();

        // 参数数量正确？
        if args.len() != 3 {
            self.reply_n_log(
                &format!("指令参数数量错误。需要3，实际为{}", args.len()),
                received_msg,
            )
            .await;
            return;
        }

        // 用户有效吗？
        let user = match self.clerk.get_user(args[0]) {
            Ok(u) => u,
            Err(e) => {
                self.reply_n_log(&format!("无法找到用户。{}", e), received_msg)
                    .await;
                return;
            }
        };

        // 指令内容时什么，及如何回复？
        let sys_reply = match &args[..] {
            [_, "充值", value] => {
                let Ok(v) = value.parse::<f64>() else {
                    self.reply_n_log("用户余额解析出错", received_msg).await;
                    return;
                };
                let user_to_update = Guest {
                    credit: user.credit + v,
                    ..user
                };
                match self.clerk.update_user(&user_to_update) {
                    Err(e) => format!("更新用户余额出错：{e}"),
                    Ok(_) => format!("更新成功。当前余额：{}", user_to_update.credit),
                }
            }
            [_, "管理员", value] => {
                let v = match value.parse::<bool>() {
                    Err(e) => {
                        self.reply_n_log(&format!("管理员属性解析出错：{e}"), received_msg)
                            .await;
                        return;
                    }
                    Ok(v) => v,
                };
                let user_to_update = Guest { admin: v, ..user };
                match self.clerk.update_user(&user_to_update) {
                    Err(e) => format!("更新管理员属性出错：{e}"),
                    Ok(_) => format!("更新成功。当前管理员属性：{}", user_to_update.admin),
                }
            }
            _ => "未知指令".to_string(),
        };
        self.reply_n_log(&sys_reply, received_msg).await;
    }

    // 处理常规用户消息
    async fn handle_guest_msg(&self, guest: &Guest, received_msg: &ReceivedMsg) {
        // 用户账户有效？
        if guest.credit <= 0.0 {
            tracing::warn!("余额不足。账户{}欠款{}。", guest.name, guest.credit.abs());
            // 告知用户欠款详情
            self.reply_n_log(
                &format!("余额不足。当前账户欠款：{}。", guest.credit.abs()),
                received_msg,
            )
            .await;
            return;
        }

        // 获取当前Assistant
        let Ok(agent_id) = received_msg.agent_id.parse::<i32>() else {
            self.reply_n_log(&format!("转换AgentID失败"), received_msg)
                .await;
            return;
        };
        let assistant = match self.clerk.get_assistant_by_agent_id(agent_id) {
            Err(e) => {
                self.reply_n_log(&format!("获取Assistant失败：{e}"), received_msg)
                    .await;
                return;
            }
            Ok(a) => a,
        };

        // 账户OK，获取用户会话记录。若会话记录不存在，则创建新记录。
        let conversation: Conversation = match self.clerk.get_conversation(guest) {
            Err(e) => {
                tracing::warn!(
                    "获取用户{}会话记录失败：{}。将为此用户创建新记录。",
                    guest.name,
                    e
                );
                match self.clerk.create_conversation(guest, &assistant) {
                    Err(e) => {
                        self.reply_n_log(
                            &format!("创建用户{}会话记录失败：{}。", guest.name, e),
                            received_msg,
                        )
                        .await;
                        return;
                    }
                    Ok(_) => {
                        tracing::info!("已为用户{}创建会话记录。", guest.name);
                        Conversation {
                            content: Vec::<Message>::new(),
                        }
                    }
                }
            }
            Ok(c) => c,
        };

        // 是指令消息吗？指令消息不会计入会话记录。
        let user_msg = received_msg.content.as_str();
        if user_msg.starts_with('#') {
            let reply_content: String = match user_msg {
                "#查余额" => format!("当前余额：{}", guest.credit),
                "#查消耗" => format!(
                    "当前会话累计消耗token{}个，费用{}。",
                    conversation.tokens(),
                    conversation.cost()
                ),
                "#新会话" => match self.clerk.create_conversation(guest, &assistant) {
                    Err(e) => format!("新建会话记录失败：{e}, {}", guest.name),
                    Ok(_) => "新会话创建成功。您可以开始对话了。".to_string(),
                },
                &_ => "抱歉，暂不支持当前指令。".to_string(),
            };
            self.reply_n_log(&reply_content, received_msg).await;
            return;
        }

        // 记录用户消息，并与当前会话记录关联
        let new_msg = Message {
            content: received_msg.content.clone(),
            role: MessageRole::User,
            cost: 0.0,
            tokens: 0,
        };
        if let Err(e) = self.clerk.append_message(guest, &new_msg) {
            self.reply_n_log(
                &format!("新增消息记录失败：{}, {e}", guest.name),
                received_msg,
            )
            .await;
            return;
        }

        // 获取AI可以处理的会话记录。
        let conv_after_update = match self.clerk.get_conversation(guest) {
            Err(e) => {
                self.reply_n_log(
                    &format!("获取会话记录失败：{}, {e}", guest.name),
                    received_msg,
                )
                .await;
                return;
            }
            Ok(c) => c,
        };

        // 交由AI处理
        let response = match self.ai_agent.chat(&conv_after_update).await {
            Ok(r) => r,
            Err(e) => {
                // 告知用户发生内部错误，避免用户徒劳重试或者等待
                self.reply_n_log(
                    &format!("获取AI回复时发生错误。请等一分钟再试，或者向管理员寻求帮助。{e}"),
                    received_msg,
                )
                .await;
                return;
            }
        };

        // 更新AI回复到会话记录
        let ai_reply = Message {
            content: response.content().to_owned(),
            role: response.role(),
            cost: response.cost(),
            tokens: response.tokens(),
        };
        if let Err(e) = self.clerk.append_message(guest, &ai_reply) {
            self.reply_n_log(
                &format!("添加消息到会话记录失败：{}, {e}", guest.name),
                received_msg,
            )
            .await;
            return;
        }

        // 扣除相应金额
        let guest_to_update = &mut guest.clone();
        guest_to_update.credit -= ai_reply.cost;
        if let Err(e) = self.clerk.update_user(guest_to_update) {
            self.reply_n_log(
                &format!("更新用户账户失败：{}, {e}", guest_to_update.name),
                received_msg,
            )
            .await;
            return;
        }

        // 回复用户最终结果
        let content = WecomText::new(ai_reply.content);
        if let Err(e) = self.reply(received_msg, content).await {
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

impl std::error::Error for Error {}
