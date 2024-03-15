use super::schema;
use crate::reception::core::{AIConversation, AIMessage, MessageRole};
use chrono::NaiveDateTime;
use diesel::prelude::*;
use std::convert::From;

// 数据库初始化状态
#[derive(Queryable, Selectable, Identifiable, PartialEq, Debug)]
#[diesel(table_name = schema::db_init_status)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct DbStatus {
    pub id: i32,
    pub initialized_at: NaiveDateTime,
}

// Guest为人类用户
#[derive(Queryable, Selectable, Identifiable, PartialEq, Debug)]
#[diesel(table_name = schema::guests)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Guest {
    pub id: i32,
    pub name: String,
    pub credit: f64,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub admin: bool,
}

#[derive(Insertable)]
#[diesel(table_name = schema::guests)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct NewGuest<'a> {
    pub name: &'a str,
    pub credit: f64,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

// Provider为AI供应商
#[derive(Queryable, Selectable, Identifiable, PartialEq, Debug)]
#[diesel(table_name = schema::providers)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Provider {
    pub id: i32,
    pub name: String,
    pub endpoint: String,
    pub max_tokens: i32,
    pub prompt_token_price: f64,
    pub completion_token_price: f64,
}

// 消息角色
#[derive(Queryable, Selectable, Identifiable, PartialEq, Debug)]
#[diesel(table_name = schema::msg_types)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct MessageType {
    pub id: i32,
    pub name: String,
}

impl From<MessageRole> for MessageType {
    fn from(value: MessageRole) -> Self {
        let (id, name) = match value {
            MessageRole::System => (1, "system"),
            MessageRole::User => (2, "user"),
            MessageRole::Assistant => (3, "assistant"),
            MessageRole::Supplementary => (4, "supplementary"),
        };
        Self {
            id,
            name: name.to_string(),
        }
    }
}

// 消息内容类型
#[derive(Queryable, Selectable, Identifiable, PartialEq, Debug)]
#[diesel(table_name = schema::content_types)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct ContentType {
    pub id: i32,
    pub name: String,
}

// Assistant为AI助手
#[derive(Queryable, Selectable, Identifiable, PartialEq, Debug)]
#[diesel(table_name = schema::assistants)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Assistant {
    pub id: i32,
    pub name: String,
    pub agent_id: i32,
    pub provider_id: i32,
}

// 会话记录
#[derive(Queryable, Selectable, Identifiable, Associations, PartialEq, Debug)]
#[diesel(table_name = schema::conversations)]
#[diesel(belongs_to(Guest))]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Conversation {
    pub id: i32,
    pub guest_id: i32,
    pub assistant_id: i32,
    pub active: bool,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Insertable)]
#[diesel(table_name = schema::conversations)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct NewConversation {
    pub guest_id: i32,
    pub assistant_id: i32,
    pub active: bool,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

// 单条会话消息
#[derive(Queryable, Selectable, Identifiable, Associations, PartialEq, Debug)]
#[diesel(table_name = schema::messages)]
#[diesel(belongs_to(Conversation))]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Message {
    pub id: i32,
    pub conversation_id: i32,
    pub created_at: NaiveDateTime,
    pub content: String,
    pub cost: f64,
    pub message_type: i32,
    pub content_type: i32,
    pub prompt_tokens: i32,
    pub completion_tokens: i32,
}

impl AIMessage for Message {
    fn content(&self) -> &str {
        &self.content
    }

    fn role(&self) -> MessageRole {
        match self.message_type {
            1 => MessageRole::System,
            2 => MessageRole::User,
            3 => MessageRole::Assistant,
            _ => MessageRole::Supplementary,
        }
    }

    fn cost(&self) -> f64 {
        0.0
    }
}

// 用于插入表的新消息
#[derive(Insertable)]
#[diesel(table_name = schema::messages)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct NewMessage {
    pub conversation_id: i32,
    pub created_at: NaiveDateTime,
    pub content: String,
    pub cost: f64,
    pub message_type: i32,
    pub content_type: i32,
    pub prompt_tokens: i32,
    pub completion_tokens: i32,
}
