/// Accountant专职用户账户管理
use crate::core::Guest;
use crate::storage::Agent as StorageAgent;
use std::fmt;
use std::sync::Arc;

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

pub struct Accountant {
    // 管理涉及到账户信息的数据库读取与更新。
    storage: Arc<StorageAgent>,
}

impl Accountant {
    pub fn new(storage: Arc<StorageAgent>) -> Self {
        Self { storage }
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
