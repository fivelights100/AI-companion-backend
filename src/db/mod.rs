pub mod ledger;
pub mod schedules;

pub async fn initialize_schema(pool: &sqlx::PgPool) -> Result<(), sqlx::Error> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS schedules (
          id SERIAL PRIMARY KEY,
          title TEXT NOT NULL,
          event_date DATE NOT NULL,
          event_time TIME NULL,
          location TEXT NULL,
          memo TEXT NULL
        )"
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS ledger_entries (
          id SERIAL PRIMARY KEY,
          entry_date DATE NOT NULL,
          entry_time TIME NULL,
          entry_type TEXT NOT NULL CHECK (entry_type IN ('income', 'expense')),
          amount BIGINT NOT NULL CHECK (amount >= 0),
          category TEXT NOT NULL,
          place TEXT NULL,
          people TEXT NULL,
          is_settled BOOLEAN NULL,
          memo TEXT NULL,
          created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
          updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )"
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_ledger_entries_date
         ON ledger_entries (entry_date DESC, entry_time DESC NULLS LAST)"
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_ledger_entries_type
         ON ledger_entries (entry_type)"
    )
    .execute(pool)
    .await?;

    Ok(())
}
