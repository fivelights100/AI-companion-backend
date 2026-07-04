use chrono::{NaiveDate, NaiveTime};

use crate::models::ledger::{CreateLedgerEntry, LedgerEntry};

pub async fn list_ledger_entries(pool: &sqlx::PgPool) -> Result<Vec<LedgerEntry>, sqlx::Error> {
    sqlx::query_as::<_, LedgerEntry>(
        "SELECT id, entry_date, entry_time, entry_type, amount, category, place, people, is_settled, memo
         FROM ledger_entries
         ORDER BY entry_date DESC, entry_time DESC NULLS LAST, id DESC"
    )
    .fetch_all(pool)
    .await
}

pub async fn create_ledger_entry(
    pool: &sqlx::PgPool,
    payload: &CreateLedgerEntry,
) -> Result<i32, sqlx::Error> {
    sqlx::query_scalar::<_, i32>(
        "INSERT INTO ledger_entries (entry_date, entry_time, entry_type, amount, category, place, people, is_settled, memo)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
         RETURNING id"
    )
    .bind(payload.entry_date)
    .bind(payload.entry_time)
    .bind(&payload.entry_type)
    .bind(payload.amount)
    .bind(&payload.category)
    .bind(&payload.place)
    .bind(&payload.people)
    .bind(payload.is_settled)
    .bind(&payload.memo)
    .fetch_one(pool)
    .await
}

pub async fn delete_ledger_entry_by_id(pool: &sqlx::PgPool, id: i32) -> Result<u64, sqlx::Error> {
    let rows_affected = sqlx::query("DELETE FROM ledger_entries WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?
        .rows_affected();

    Ok(rows_affected)
}

pub async fn add_ledger_entry(
    pool: &sqlx::PgPool,
    entry_date: NaiveDate,
    entry_time: Option<NaiveTime>,
    entry_type: &str,
    amount: i64,
    category: &str,
    place: Option<&str>,
    people: Option<&str>,
    is_settled: Option<bool>,
    memo: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO ledger_entries (entry_date, entry_time, entry_type, amount, category, place, people, is_settled, memo)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)"
    )
    .bind(entry_date)
    .bind(entry_time)
    .bind(entry_type)
    .bind(amount)
    .bind(category)
    .bind(place)
    .bind(people)
    .bind(is_settled)
    .bind(memo)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn get_ledger_entries(pool: &sqlx::PgPool) -> Result<Vec<String>, sqlx::Error> {
    let rows: Vec<(NaiveDate, Option<NaiveTime>, String, i64, String, Option<String>, Option<String>, Option<bool>, Option<String>)> = sqlx::query_as(
        "SELECT entry_date, entry_time, entry_type, amount, category, place, people, is_settled, memo
         FROM ledger_entries
         ORDER BY entry_date DESC, entry_time DESC NULLS LAST, id DESC
         LIMIT 50"
    )
    .fetch_all(pool)
    .await?;

    let entries = rows
        .into_iter()
        .map(|(entry_date, entry_time, entry_type, amount, category, place, people, is_settled, memo)| {
            let type_label = if entry_type == "income" { "수입" } else { "지출" };
            let time = entry_time
                .map(|value| value.format("%H:%M").to_string())
                .unwrap_or_default();
            let place = place.unwrap_or_else(|| "-".to_string());
            let people = people.unwrap_or_else(|| "-".to_string());
            let settled = match is_settled {
                Some(true) => "정산 완료",
                Some(false) => "미정산",
                None => "해당 없음",
            };
            let memo = memo.unwrap_or_else(|| "-".to_string());

            format!(
                "- [{type_label}] {amount}원 / {category} / {entry_date} {time} / 장소: {place} / 인원: {people} / 정산: {settled} / 메모: {memo}"
            )
        })
        .collect();

    Ok(entries)
}

pub async fn delete_ledger_entry(pool: &sqlx::PgPool, keyword: &str) -> Result<u64, sqlx::Error> {
    let search_pattern = format!("%{}%", keyword);

    let rows_affected = sqlx::query(
        "DELETE FROM ledger_entries
         WHERE category LIKE $1
            OR COALESCE(place, '') LIKE $1
            OR COALESCE(people, '') LIKE $1
            OR COALESCE(memo, '') LIKE $1"
    )
    .bind(search_pattern)
    .execute(pool)
    .await?
    .rows_affected();

    Ok(rows_affected)
}
