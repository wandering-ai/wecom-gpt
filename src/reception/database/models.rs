use super::schema;
use chrono::NaiveDateTime;
use diesel::prelude::*;

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
}

#[derive(Insertable)]
#[diesel(table_name = schema::guests)]
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
