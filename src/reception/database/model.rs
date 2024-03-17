use super::schema;
use chrono::NaiveDateTime;
use diesel::prelude::*;

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
    pub admin: bool,
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
    pub tokens: i32,
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
    pub tokens: i32,
}
