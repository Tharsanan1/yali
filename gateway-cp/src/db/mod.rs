use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};

mod policies;
mod routes;

pub use policies::{get_policy, get_policy_version, insert_policy, list_policies};
pub use routes::{delete_route, get_route, insert_route, list_routes, update_route};

pub async fn connect(database_url: &str) -> Result<SqlitePool, sqlx::Error> {
    SqlitePoolOptions::new()
        .max_connections(8)
        .connect(database_url)
        .await
}

pub async fn init(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS policies (
            id TEXT NOT NULL,
            version TEXT NOT NULL,
            wasm_uri TEXT NOT NULL,
            sha256 TEXT NOT NULL,
            config_json TEXT NOT NULL,
            supported_stages_json TEXT NOT NULL DEFAULT '["pre_route","pre_upstream","post_response"]',
            config_schema_json TEXT NOT NULL DEFAULT '{"type":"object"}',
            default_config_json TEXT NOT NULL DEFAULT '{}',
            created_at INTEGER NOT NULL,
            PRIMARY KEY (id, version)
        );
        "#,
    )
    .execute(pool)
    .await?;

    migrate_policies_table(pool).await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS routes (
            id TEXT NOT NULL PRIMARY KEY,
            match_json TEXT NOT NULL,
            upstreams_json TEXT NOT NULL,
            lb TEXT,
            failover_json TEXT,
            policies_json TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        );
        "#,
    )
    .execute(pool)
    .await?;

    Ok(())
}

async fn migrate_policies_table(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    if !column_exists(pool, "policies", "supported_stages_json").await? {
        sqlx::query(
            r#"ALTER TABLE policies ADD COLUMN supported_stages_json TEXT NOT NULL DEFAULT '["pre_route","pre_upstream","post_response"]'"#,
        )
        .execute(pool)
        .await?;
    }

    if !column_exists(pool, "policies", "config_schema_json").await? {
        sqlx::query(
            r#"ALTER TABLE policies ADD COLUMN config_schema_json TEXT NOT NULL DEFAULT '{"type":"object"}'"#,
        )
        .execute(pool)
        .await?;
    }

    if !column_exists(pool, "policies", "default_config_json").await? {
        sqlx::query(
            r#"ALTER TABLE policies ADD COLUMN default_config_json TEXT NOT NULL DEFAULT '{}'"#,
        )
        .execute(pool)
        .await?;
    }

    // Backfill defaults from legacy config_json if present.
    sqlx::query(
        r#"
        UPDATE policies
        SET default_config_json = config_json
        WHERE default_config_json IN ('{}', 'null', '')
          AND config_json NOT IN ('null', '')
        "#,
    )
    .execute(pool)
    .await?;

    Ok(())
}

async fn column_exists(pool: &SqlitePool, table: &str, column: &str) -> Result<bool, sqlx::Error> {
    use sqlx::Row;

    let rows = sqlx::query(&format!("PRAGMA table_info({table})"))
        .fetch_all(pool)
        .await?;

    Ok(rows.iter().any(|row| {
        row.try_get::<String, _>("name")
            .map(|name| name == column)
            .unwrap_or(false)
    }))
}

fn current_ts() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
