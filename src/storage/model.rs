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
