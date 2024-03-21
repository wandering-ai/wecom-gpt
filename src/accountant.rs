//! Accountant专职用户账户管理
use crate::core::Guest;
use crate::storage::Agent as StorageAgent;
use crate::wecom_api::{CallbackParams, CallbackRequestBody, ContactEventContent, UrlVerifyParams};
use axum::extract::Query;
use serde::Deserialize;
use serde_xml_rs::from_str;
use std::fmt;
use std::sync::Arc;
use wecom_crypto::Agent as CryptoAgent;

#[derive(Debug)]
pub enum Error {
    NotFound,
    Overdue(f64),
    Internal(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let err_msg = match self {
            Self::NotFound => "账户不存在",
            Self::Overdue(_) => "账户欠款",
            Self::Internal(s) => s,
        };
        write!(f, "{}", err_msg)
    }
}

#[derive(Deserialize, Clone)]
pub struct Config {
    pub agent_id: u64,
    pub token: String,
    pub key: String,
}

// 账户信息的数据库读取与更新。
pub struct Accountant {
    agent_id: u64,
    storage: Arc<StorageAgent>,
    crypto_agent: CryptoAgent,
}

impl Accountant {
    pub fn new(storage: Arc<StorageAgent>, config: &Config) -> Self {
        let crypto_agent = CryptoAgent::new(&config.token, &config.key);
        Self {
            agent_id: config.agent_id,
            storage,
            crypto_agent,
        }
    }

    /// 返回当前企业微信通讯录应用对应的ID
    pub fn agent_id(&self) -> u64 {
        self.agent_id
    }

    /// 通讯录API服务有效性验证
    pub fn verify_url(&self, params: &UrlVerifyParams) -> Result<String, Error> {
        if self.crypto_agent.generate_signature(vec![
            &params.timestamp,
            &params.nonce,
            &params.echostr,
        ]) != params.msg_signature
        {
            return Err(Error::Internal("签名校验失败".to_string()));
        }
        Ok(self
            .crypto_agent
            .decrypt(&params.echostr)
            .map_err(|e| Error::Internal(format!("解密消息失败。{e}")))?
            .text)
    }

    /// 处理企业微信发来的新增用户事件
    pub fn handle_user_creation_event(
        &self,
        params: Query<CallbackParams>,
        body: String,
    ) -> Result<(), Error> {
        // 获取请求Body结构体
        let body: CallbackRequestBody =
            from_str(&body).map_err(|e| Error::Internal(format!("解析Body出错。{e}")))?;

        // 消息被篡改？
        if self.crypto_agent.generate_signature(vec![
            &params.timestamp,
            &params.nonce,
            &body.encrypted_str,
        ]) != params.msg_signature
        {
            return Err(Error::Internal(
                "签名校验失败。数据可能被篡改。".to_string(),
            ));
        }

        // 加密的内容是什么？
        let decrypt_result = self
            .crypto_agent
            .decrypt(&body.encrypted_str)
            .map_err(|e| Error::Internal(format!("解密用户数据失败。{e}")))?;
        let callback_content = from_str::<ContactEventContent>(&decrypt_result.text)
            .map_err(|e| Error::Internal(format!("解析xml失败。{e}")))?;
        tracing::debug!("Callback parsed");

        // 注册该用户
        let guest = Guest {
            name: callback_content.user_id,
            credit: 0.0,
            admin: false,
        };
        self.register(&guest)
            .map_err(|e| Error::Internal(format!("新增用户失败。{e}")))
    }

    /// 开户
    pub fn register(&self, guest: &Guest) -> Result<(), Error> {
        self.storage
            .create_user(guest)
            .map_err(|e| Error::Internal(format!("新建用户失败。用户名：{}， {e}", guest.name)))
    }

    /// 检查账户的有效性
    pub fn verify_guest(&self, guest_name: &str) -> Result<(), Error> {
        let user = self
            .storage
            .get_user(guest_name)
            .map_err(|_| Error::NotFound)?;

        if user.credit <= 0.0 {
            Err(Error::Overdue(user.credit))
        } else {
            Ok(())
        }
    }

    /// 获取账户。若不存在则触发NotFound错误。
    pub fn get_guest(&self, guest_name: &str) -> Result<Guest, Error> {
        self.storage
            .get_user(guest_name)
            .map_err(|_| Error::NotFound)
    }

    /// 更新账户
    pub fn update_guest(&self, guest: &Guest) -> Result<(), Error> {
        self.storage
            .update_user(guest)
            .map_err(|e| Error::Internal(format!("更新用户失败。{e}")))
    }
}
