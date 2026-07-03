use chrono::{NaiveDate, NaiveTime};

use crate::models::schedule::{CreateSchedule, Schedule};

pub async fn list_schedules(pool: &sqlx::PgPool) -> Result<Vec<Schedule>, sqlx::Error> {
    sqlx::query_as::<_, Schedule>(
        "SELECT id, title, event_date, event_time, location, memo
         FROM schedules
         ORDER BY event_date ASC, event_time ASC NULLS LAST"
    )
    .fetch_all(pool)
    .await
}

pub async fn create_schedule(
    pool: &sqlx::PgPool,
    payload: &CreateSchedule,
) -> Result<i32, sqlx::Error> {
    sqlx::query_scalar::<_, i32>(
        "INSERT INTO schedules (title, event_date, event_time, location, memo)
         VALUES ($1, $2, $3, $4, $5)
         RETURNING id"
    )
    .bind(&payload.title)
    .bind(payload.event_date)
    .bind(payload.event_time)
    .bind(&payload.location)
    .bind(&payload.memo)
    .fetch_one(pool)
    .await
}

pub async fn delete_schedule_by_id(pool: &sqlx::PgPool, id: i32) -> Result<u64, sqlx::Error> {
    let rows_affected = sqlx::query("DELETE FROM schedules WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?
        .rows_affected();

    Ok(rows_affected)
}

pub async fn add_schedule(
    pool: &sqlx::PgPool,
    title: &str,
    event_date: NaiveDate,
    event_time: Option<NaiveTime>,
    location: Option<&str>,
    memo: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO schedules (title, event_date, event_time, location, memo)
         VALUES ($1, $2, $3, $4, $5)"
    )
    .bind(title)
    .bind(event_date)
    .bind(event_time)
    .bind(location)
    .bind(memo)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn get_schedules(pool: &sqlx::PgPool) -> Result<Vec<String>, sqlx::Error> {
    let rows: Vec<(String, NaiveDate, Option<NaiveTime>)> = sqlx::query_as(
        "SELECT title, event_date, event_time
         FROM schedules
         ORDER BY event_date ASC, event_time ASC NULLS LAST"
    )
    .fetch_all(pool)
    .await?;

    let schedules = rows
        .into_iter()
        .map(|(title, event_date, event_time)| {
            let time = event_time
                .map(|value| value.to_string())
                .unwrap_or_default();
            format!("- {} ({} {})", title, event_date, time)
        })
        .collect();

    Ok(schedules)
}

pub async fn delete_schedule(pool: &sqlx::PgPool, keyword: &str) -> Result<u64, sqlx::Error> {
    let search_pattern = format!("%{}%", keyword);

    let rows_affected = sqlx::query("DELETE FROM schedules WHERE title LIKE $1")
        .bind(search_pattern)
        .execute(pool)
        .await?
        .rows_affected();

    Ok(rows_affected)
}
