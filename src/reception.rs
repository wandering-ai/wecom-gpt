//! Agent负责用户管理，用户请求预处理与分发，收集AI反馈并返回给用户。
use axum::extract::Query;
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use serde_xml_rs::from_str;
use std::collections::HashMap;
use std::env;
use std::fmt;
use std::sync::Arc;

// 企业微信加解密模块
use wecom_crypto::Agent as CryptoAgent;

// 企业微信消息发送模块
use wecom_agent::{
    message::{MessageBuilder as WecomMsgBuilder, Text as WecomText, WecomMessage},
    WecomAgent,
};

// 企业微信服务端业务解析模块
use super::wecom_api::{AppMessageContent, CallbackParams, CallbackRequestBody, UrlVerifyParams};

// 用户管理模块
use super::accountant::{Accountant, Config as AccountantCfg, Error as AccountError};

// 人工智能模块
use super::assistant::{Assistant, Config as AssistantCfg, ProviderCfg};

// 存储模块
use super::storage::Agent as StorageAgent;

// 交互涉及到的核心概念
use super::core::{Chat, ChatResponse, Guest};

#[derive(Debug, Clone)]
pub struct Error(String);
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
impl std::error::Error for Error {}

// 初始化应用所需要的配置项。这些配置项内容将从配置文件中读取。
#[derive(Deserialize, Clone)]
pub struct Config {
    wecom: WecomCfg,
    providers: Vec<ProviderCfg>,
    assistants: Vec<AssistantCfg>,
    accountant: AccountantCfg,
    storage_path: String,
    admin_account: String,
}

// 企业微信服务所需要的参数
#[derive(Deserialize, Clone)]
pub struct WecomCfg {
    corp_id: String,
}

/// Agent负责协调用户与AI之间的交互过程
pub struct Agent {
    assistants: HashMap<u64, Assistant>,      // 负责AI功能
    crypto_agents: HashMap<u64, CryptoAgent>, // 负责企业微信消息加解密
    messengers: HashMap<u64, WecomAgent>,     // 负责消息传递
    accountant: Accountant,                   // 负责账户管理
}

// 转换环境变量解析错误
fn to_local_err(name: &str) -> Error {
    Error(format!("找不到环境变量{name}"))
}

impl Agent {
    /// 新建一个应用Agent
    pub fn new(config: &Config) -> Result<Self, Error> {
        // 初始化存储模块
        let admin_name =
            env::var(&config.admin_account).map_err(|_| to_local_err(&config.admin_account))?;
        let storage = Arc::new(
            StorageAgent::new(&config.storage_path, admin_name.as_str())
                .map_err(|e| Error(format!("数据库初始化失败。{e}")))?,
        );

        // 初始化Assistant、加解密与消息模块
        let mut crypto_agents: HashMap<u64, CryptoAgent> = HashMap::new();
        let mut assistants: HashMap<u64, Assistant> = HashMap::new();
        let mut messengers: HashMap<u64, WecomAgent> = HashMap::new();

        for assis_cfg in &config.assistants {
            let mut a_cfg = assis_cfg.clone();
            // 加解密模块
            a_cfg.token = env::var(&assis_cfg.token).map_err(|_| to_local_err(&assis_cfg.token))?;
            a_cfg.key = env::var(&assis_cfg.key).map_err(|_| to_local_err(&assis_cfg.key))?;
            crypto_agents.insert(a_cfg.agent_id, CryptoAgent::new(&a_cfg.token, &a_cfg.key));

            // 消息发送模块
            let corp_id =
                env::var(&config.wecom.corp_id).map_err(|_| to_local_err(&config.wecom.corp_id))?;
            a_cfg.secret = env::var(&a_cfg.secret).map_err(|_| to_local_err(&a_cfg.secret))?;
            messengers.insert(a_cfg.agent_id, WecomAgent::new(&corp_id, &a_cfg.secret));

            // 匹配的AI是哪一个
            for provider_cfg in &config.providers {
                if provider_cfg.id == assis_cfg.provider_id {
                    let mut p_cfg = provider_cfg.clone();
                    p_cfg.endpoint =
                        env::var(&p_cfg.endpoint).map_err(|_| to_local_err(&p_cfg.endpoint))?;
                    p_cfg.api_key =
                        env::var(&p_cfg.api_key).map_err(|_| to_local_err(&p_cfg.api_key))?;
                    assistants.insert(
                        a_cfg.agent_id,
                        Assistant::new(&a_cfg, &p_cfg, storage.clone()),
                    );
                }
            }
        }

        // 账户管理模块
        let mut acct_cfg = config.accountant.clone();
        acct_cfg.token = env::var(&acct_cfg.token).map_err(|_| to_local_err(&acct_cfg.token))?;
        acct_cfg.key = env::var(&acct_cfg.key).map_err(|_| to_local_err(&acct_cfg.key))?;
        let accountant = Accountant::new(storage.clone(), &acct_cfg);

        Ok(Self {
            assistants,
            crypto_agents,
            messengers,
            accountant,
        })
    }

    /// 配合企业微信，验证服务器地址的有效性。
    pub fn verify_url(
        &self,
        agent_id: u64,
        params: Query<UrlVerifyParams>,
    ) -> Result<String, StatusCode> {
        // 验证的是通讯录组件吗？
        if agent_id == self.accountant.agent_id() {
            return self.accountant.verify_url(&params).map_err(|e| {
                tracing::error!("校验URL失败。{e}");
                StatusCode::BAD_REQUEST
            });
        }

        // 验证对象是哪个Assistant？
        let Some(crypto_agent) = self.crypto_agents.get(&agent_id) else {
            tracing::error!("无法获得加解密对象。agent_id: {agent_id}");
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        };

        // Is this request safe?
        if crypto_agent.generate_signature(vec![&params.timestamp, &params.nonce, &params.echostr])
            != params.msg_signature
        {
            tracing::error!("校验签名失败");
            return Err(StatusCode::BAD_REQUEST);
        }

        // Give the server what it expects.
        Ok(crypto_agent
            .decrypt(&params.echostr)
            .map_err(|e| {
                tracing::error!("解密消息失败。{e}");
                StatusCode::INTERNAL_SERVER_ERROR
            })?
            .text)
    }

    /// 处理用户发来的请求
    /// 目前应用的管理操作同样使用本接口来实现。故需按照用户角色与内容来协同判断用户请求的意图。
    pub async fn handle_user_request(
        &self,
        agent_id: u64,
        params: Query<CallbackParams>,
        body: String,
    ) {
        // 获取请求Body结构体
        let body: CallbackRequestBody = match from_str(&body) {
            Err(e) => {
                tracing::error!("[{agent_id}] 解析Body出错。终止当前操作。{e}");
                return;
            }
            Ok(b) => b,
        };

        // 谁可以校验此请求？
        let Some(crypto_agent) = self.crypto_agents.get(&agent_id) else {
            tracing::error!("[{agent_id}] 加解密代理不存在。终止当前操作。");
            return;
        };

        // 消息被篡改？
        if crypto_agent.generate_signature(vec![
            &params.timestamp,
            &params.nonce,
            &body.encrypted_str,
        ]) != params.msg_signature
        {
            tracing::error!("[{agent_id}] 签名校验失败。数据可能被篡改。终止当前操作。");
            return;
        }

        // 加密的内容是什么？
        let decrypt_result = match crypto_agent.decrypt(&body.encrypted_str) {
            Err(e) => {
                tracing::error!("[{agent_id}] 解密用户数据失败。终止当前操作。{e}");
                return;
            }
            Ok(x) => x,
        };
        let msg_content = match from_str::<AppMessageContent>(&decrypt_result.text) {
            Err(e) => {
                tracing::error!("[{agent_id}] 解析xml失败。终止当前操作。{e}");
                return;
            }
            Ok(x) => x,
        };
        tracing::debug!("User message parsed");

        // 首先验证消息发送者。若用户不存在，则尝试创建该用户。若用户逾期，则返回具体金额。
        let guest_name: &str = msg_content.from_user_name.as_str();
        let overdue: f64 = match self.accountant.verify_guest(guest_name) {
            Err(AccountError::Internal(e)) => {
                tracing::error!("[{agent_id}] 验证用户失败。终止当前操作。{e}");
                return;
            }
            Err(AccountError::Overdue(credit)) => credit,
            Err(AccountError::NotFound) => {
                tracing::warn!("[{agent_id}] 用户不存在。将注册用户：{guest_name}");
                let new_guest = Guest {
                    name: guest_name.to_owned(),
                    credit: 0.0,
                    admin: false,
                };
                if let Err(e) = self.accountant.register(&new_guest) {
                    tracing::error!("[{agent_id}] 注册用户失败。终止当前操作。{e}");
                    return;
                }
                tracing::info!("[{agent_id}] 注册用户成功：{guest_name}");
                0.0
            }
            Ok(_) => 0.0,
        };
        let Ok(guest) = self.accountant.get_guest(guest_name) else {
            tracing::error!("[{agent_id}] 获取用户失败。终止当前操作。");
            return;
        };

        // 是指令消息吗？指令消息需要无条件响应。
        // 管理员指令来自管理员(Guest::admin=true)，并且匹配管理员指令格式：$$指令内容$$
        // 用户指令来自普通用户(Guest::admin=false)，并且匹配用户指令格式：#指令内容
        // 所有的指令操作均需要保留日志。
        let msg_str = msg_content.content.as_str();
        if (msg_str.trim().starts_with("$$") && msg_str.trim().ends_with("$$"))
            || msg_str.starts_with('#')
        {
            tracing::debug!("[{agent_id}] Got instruct message, going to handle it..");
            let sys_msg = self.handle_instruction_msg(&guest, agent_id, &msg_content.content);
            self.log_n_reply(&sys_msg, &msg_content).await;
            return;
        }

        // 用户是否可以使用本服务？
        if overdue < 0.0 {
            self.log_n_reply(&format!("账户余额不足。当前余额{overdue:.3}"), &msg_content)
                .await;
            return;
        }

        // 谁来处理常规用户消息？
        let Some(assistant) = self.assistants.get(&agent_id) else {
            tracing::error!("[{agent_id}] 助手不存在。终止当前操作。");
            return;
        };
        let reply_msg = match assistant.chat(&guest, &msg_content.content).await {
            Err(e) => {
                tracing::error!("[{agent_id}] 获取AI回复失败。终止当前操作。{e}");
                return;
            }
            Ok(m) => m,
        };

        // 扣除相应金额
        let mut guest_to_update = guest.clone();
        guest_to_update.credit -= reply_msg.cost();
        if let Err(e) = self.accountant.update_guest(&guest_to_update) {
            tracing::error!(
                "[{agent_id}] 更新用户账户失败。终止当前操作。{}, {e}",
                guest.name
            );
            return;
        }
        tracing::debug!(
            "[{agent_id}] User {} charged {}",
            guest.name,
            reply_msg.cost()
        );

        // 回复给用户
        let content = WecomText::new(reply_msg.content().to_owned());
        if let Err(e) = self.reply(content, &msg_content).await {
            tracing::error!("[{agent_id}] 回复用户消息失败。{e}");
        }
    }

    // 向用户回复一条消息。消息内容content需要满足WecomMessage。
    async fn reply<T>(&self, content: T, msg_content: &AppMessageContent) -> Result<(), Error>
    where
        T: Serialize + WecomMessage,
    {
        let agent_id = msg_content
            .agent_id
            .parse::<u64>()
            .map_err(|e| Error(format!("解析agent_id出错。{e}")))?;
        let msg = WecomMsgBuilder::default()
            .to_users(vec![&msg_content.from_user_name])
            .from_agent(agent_id as usize)
            .build(content)
            .map_err(|e| Error(format!("构建微信消息时出错。{e}")))?;

        // 发送该消息
        tracing::debug!("Sending message to {} ...", msg_content.from_user_name);
        let Some(messenger) = self.messengers.get(&agent_id) else {
            return Err(Error(format!("找不到可用的消息代理。 {agent_id}")));
        };
        let response = messenger
            .send(msg)
            .await
            .map_err(|e| Error(format!("调用发送消息API失败。{e}")))?;

        // 发送成功，但是服务器返回错误。
        if response.is_error() {
            return Err(Error(format!(
                "发送消息后收到异常信息。 {}, {}",
                response.error_code(),
                response.error_msg()
            )));
        }
        Ok(())
    }

    // 回复消息。并将消息内容记录在日志中。主要用在系统指令消息处理中。
    async fn log_n_reply(&self, msg: &str, msg_content: &AppMessageContent) {
        tracing::info!(msg);
        let content = WecomText::new(msg.to_owned());
        if let Err(e) = self.reply(content, msg_content).await {
            tracing::error!("发送系统消息时出错。{e}");
        }
    }

    // 处理指令消息
    // 管理员指令内容："用户名 操作名 操作内容"。例如"小白 充值 3.5"。
    // 常规用户指令内容："查余额"、"查消耗"、"新会话"
    fn handle_instruction_msg(
        &self,
        guest: &Guest,
        assistant_id: u64,
        instruction: &str,
    ) -> String {
        // 指令角色？
        if guest.admin && instruction.starts_with('$') {
            let msg = instruction.trim_matches('$');
            let args: Vec<&str> = msg.split(' ').collect();

            // 指令内容时什么，及如何回复？
            match &args[..] {
                ["查用户"] => {
                    let Ok(guests) = self.accountant.get_guests() else {
                        return "无法从数据库中获得用户".to_string();
                    };
                    let mut msg = String::new();
                    for g in &guests {
                        msg.push_str(format!("{} {} {}", g.name, g.credit, g.admin).as_str());
                    }
                    msg
                }
                [_, "充值", value] => {
                    let Ok(v) = value.parse::<f64>() else {
                        return "用户余额解析出错".to_string();
                    };
                    // 获取待操作的用户
                    let user = match self.accountant.get_guest(args[0]) {
                        Ok(u) => u,
                        Err(e) => return format!("无法找到用户。{e}"),
                    };
                    // 更新用户
                    let user_to_update = Guest {
                        credit: user.credit + v,
                        ..user
                    };
                    match self.accountant.update_guest(&user_to_update) {
                        Err(e) => format!("更新用户余额出错：{e}"),
                        Ok(_) => format!("更新成功。当前余额：{}", user_to_update.credit),
                    }
                }
                [_, "管理员", value] => {
                    let Ok(v) = value.parse::<bool>() else {
                        return "管理员属性解析出错。".to_string();
                    };
                    // 获取待操作的用户
                    let user = match self.accountant.get_guest(args[0]) {
                        Ok(u) => u,
                        Err(e) => return format!("无法找到用户。{e}"),
                    };
                    // 更新用户
                    let user_to_update = Guest { admin: v, ..user };
                    match self.accountant.update_guest(&user_to_update) {
                        Err(e) => format!("更新管理员属性出错：{e}"),
                        Ok(_) => format!(
                            "更新成功。{}{}",
                            user_to_update.name,
                            if user_to_update.admin {
                                "已成为管理员"
                            } else {
                                "不再是管理员"
                            }
                        ),
                    }
                }
                _ => "未知指令".to_string(),
            }
        } else {
            // 常规账户指令
            let Some(assistant) = self.assistants.get(&assistant_id) else {
                tracing::error!("助手不存在。终止当前操作。agent_id: {assistant_id}");
                return "内部错误，请稍后再试。".to_string();
            };
            match instruction {
                "#查余额" => format!("当前余额：{:.3}", guest.credit),
                "#查消耗" => assistant.audit(guest),
                "#新会话" => match assistant.new_conversation(guest) {
                    Err(e) => format!("为{}新建会话记录失败。{}", guest.name, e),
                    Ok(_) => "新会话创建成功。您可以开始对话了。".to_string(),
                },
                &_ => "抱歉，暂不支持当前指令。".to_string(),
            }
        }
    }

    /// 处理通讯录更新时间
    pub async fn handle_account_creation(&self, params: Query<CallbackParams>, body: String) {
        match self.accountant.handle_user_creation_event(params, body) {
            Err(e) => tracing::error!("处理新增用户事件失败。{e}"),
            Ok(_) => tracing::info!("新增用户成功。用户ID"),
        };
    }
}
