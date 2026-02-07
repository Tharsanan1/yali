use crate::model::PolicySpec;
use sqlx::SqlitePool;

use super::current_ts;

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

fn row_to_policy(row: sqlx::sqlite::SqliteRow) -> Result<PolicySpec, sqlx::Error> {
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
