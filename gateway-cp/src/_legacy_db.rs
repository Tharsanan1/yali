use crate::models::{PolicySpec, RouteSpec};
use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};

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

pub async fn insert_policy(pool: &SqlitePool, policy: &PolicySpec) -> Result<(), sqlx::Error> {
    let config_json = serde_json::to_string(&policy.config).unwrap_or_else(|_| "null".to_string());
    let now = current_ts();
    sqlx::query(
        r#"
        INSERT INTO policies (id, version, wasm_uri, sha256, config_json, created_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        "#,
    )
    .bind(&policy.id)
    .bind(&policy.version)
    .bind(&policy.wasm_uri)
    .bind(&policy.sha256)
    .bind(config_json)
    .bind(now)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn list_policies(pool: &SqlitePool) -> Result<Vec<PolicySpec>, sqlx::Error> {
    let rows = sqlx::query(
        r#"
        SELECT id, version, wasm_uri, sha256, config_json
        FROM policies
        ORDER BY id ASC, version ASC
        "#,
    )
    .fetch_all(pool)
    .await?;

    rows.into_iter().map(row_to_policy).collect()
}

pub async fn get_policy(
    pool: &SqlitePool,
    id: &str,
    version: Option<&str>,
) -> Result<Vec<PolicySpec>, sqlx::Error> {
    let rows = if let Some(version) = version {
        sqlx::query(
            r#"
            SELECT id, version, wasm_uri, sha256, config_json
            FROM policies
            WHERE id = ?1 AND version = ?2
            "#,
        )
        .bind(id)
        .bind(version)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query(
            r#"
            SELECT id, version, wasm_uri, sha256, config_json
            FROM policies
            WHERE id = ?1
            ORDER BY version ASC
            "#,
        )
        .bind(id)
        .fetch_all(pool)
        .await?
    };

    rows.into_iter().map(row_to_policy).collect()
}

pub async fn insert_route(pool: &SqlitePool, route: &RouteSpec) -> Result<(), sqlx::Error> {
    let match_json = serde_json::to_string(&route.match_rules).unwrap_or_else(|_| "{}".to_string());
    let upstreams_json = serde_json::to_string(&route.upstreams).unwrap_or_else(|_| "[]".to_string());
    let failover_json = serde_json::to_string(&route.failover).unwrap_or_else(|_| "null".to_string());
    let policies_json = serde_json::to_string(&route.policies).unwrap_or_else(|_| "[]".to_string());
    let now = current_ts();

    sqlx::query(
        r#"
        INSERT INTO routes (id, match_json, upstreams_json, lb, failover_json, policies_json, created_at, updated_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
        "#,
    )
    .bind(&route.id)
    .bind(match_json)
    .bind(upstreams_json)
    .bind(&route.lb)
    .bind(failover_json)
    .bind(policies_json)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn list_routes(pool: &SqlitePool) -> Result<Vec<RouteSpec>, sqlx::Error> {
    let rows = sqlx::query(
        r#"
        SELECT id, match_json, upstreams_json, lb, failover_json, policies_json
        FROM routes
        ORDER BY id ASC
        "#,
    )
    .fetch_all(pool)
    .await?;

    rows.into_iter().map(row_to_route).collect()
}

pub async fn get_route(pool: &SqlitePool, id: &str) -> Result<Option<RouteSpec>, sqlx::Error> {
    let row = sqlx::query(
        r#"
        SELECT id, match_json, upstreams_json, lb, failover_json, policies_json
        FROM routes
        WHERE id = ?1
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    row.map(row_to_route).transpose()
}

pub async fn update_route(pool: &SqlitePool, route: &RouteSpec) -> Result<u64, sqlx::Error> {
    let match_json = serde_json::to_string(&route.match_rules).unwrap_or_else(|_| "{}".to_string());
    let upstreams_json = serde_json::to_string(&route.upstreams).unwrap_or_else(|_| "[]".to_string());
    let failover_json = serde_json::to_string(&route.failover).unwrap_or_else(|_| "null".to_string());
    let policies_json = serde_json::to_string(&route.policies).unwrap_or_else(|_| "[]".to_string());
    let now = current_ts();

    let result = sqlx::query(
        r#"
        UPDATE routes
        SET match_json = ?2,
            upstreams_json = ?3,
            lb = ?4,
            failover_json = ?5,
            policies_json = ?6,
            updated_at = ?7
        WHERE id = ?1
        "#,
    )
    .bind(&route.id)
    .bind(match_json)
    .bind(upstreams_json)
    .bind(&route.lb)
    .bind(failover_json)
    .bind(policies_json)
    .bind(now)
    .execute(pool)
    .await?;

    Ok(result.rows_affected())
}

pub async fn delete_route(pool: &SqlitePool, id: &str) -> Result<u64, sqlx::Error> {
    let result = sqlx::query("DELETE FROM routes WHERE id = ?1")
        .bind(id)
        .execute(pool)
        .await?;

    Ok(result.rows_affected())
}

fn row_to_route(
    row: sqlx::sqlite::SqliteRow,
) -> Result<RouteSpec, sqlx::Error> {
    use sqlx::Row;

    let id: String = row.try_get("id")?;
    let match_json: String = row.try_get("match_json")?;
    let upstreams_json: String = row.try_get("upstreams_json")?;
    let lb: Option<String> = row.try_get("lb")?;
    let failover_json: String = row.try_get("failover_json")?;
    let policies_json: String = row.try_get("policies_json")?;

    let match_rules = serde_json::from_str(&match_json).unwrap_or(serde_json::Value::Object(Default::default()));
    let upstreams = serde_json::from_str(&upstreams_json).unwrap_or_default();
    let failover = serde_json::from_str(&failover_json).unwrap_or(None);
    let policies = serde_json::from_str(&policies_json).unwrap_or_default();

    Ok(RouteSpec {
        id,
        match_rules,
        upstreams,
        lb,
        failover,
        policies,
    })
}

fn row_to_policy(
    row: sqlx::sqlite::SqliteRow,
) -> Result<PolicySpec, sqlx::Error> {
    use sqlx::Row;

    let id: String = row.try_get("id")?;
    let version: String = row.try_get("version")?;
    let wasm_uri: String = row.try_get("wasm_uri")?;
    let sha256: String = row.try_get("sha256")?;
    let config_json: String = row.try_get("config_json")?;

    let config = serde_json::from_str(&config_json).unwrap_or(serde_json::Value::Null);

    Ok(PolicySpec {
        id,
        version,
        wasm_uri,
        sha256,
        config,
    })
}

fn current_ts() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
