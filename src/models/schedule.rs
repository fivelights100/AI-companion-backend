use chrono::{NaiveDate, NaiveTime};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct Schedule {
    pub id: i32,
    pub title: String,
    pub event_date: NaiveDate,
    pub event_time: Option<NaiveTime>,
    pub location: Option<String>,
    pub memo: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateSchedule {
    pub title: String,
    pub event_date: NaiveDate,
    pub event_time: Option<NaiveTime>,
    pub location: Option<String>,
    pub memo: Option<String>,
}
