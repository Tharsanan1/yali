use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};

mod policies;
mod routes;

pub use policies::{get_policy, insert_policy, list_policies};
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
            created_at INTEGER NOT NULL,
            PRIMARY KEY (id, version)
        );
        "#,
    )
    .execute(pool)
    .await?;

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

fn current_ts() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
