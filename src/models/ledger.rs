use chrono::{NaiveDate, NaiveTime};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct LedgerEntry {
    pub id: i32,
    pub entry_date: NaiveDate,
    pub entry_time: Option<NaiveTime>,
    pub entry_type: String,
    pub amount: i64,
    pub category: String,
    pub place: Option<String>,
    pub people: Option<String>,
    pub is_settled: Option<bool>,
    pub memo: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateLedgerEntry {
    pub entry_date: NaiveDate,
    pub entry_time: Option<NaiveTime>,
    pub entry_type: String,
    pub amount: i64,
    pub category: String,
    pub place: Option<String>,
    pub people: Option<String>,
    pub is_settled: Option<bool>,
    pub memo: Option<String>,
}
