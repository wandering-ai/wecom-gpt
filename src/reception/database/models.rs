use super::schema;
use chrono::NaiveDateTime;
use diesel::prelude::*;

#[derive(Queryable, Selectable)]
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
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct NewGuest<'a> {
    pub name: &'a str,
    pub credit: f64,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}
