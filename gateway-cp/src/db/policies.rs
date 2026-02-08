use crate::model::PolicySpec;
use sqlx::SqlitePool;

use super::current_ts;

pub async fn insert_policy(pool: &SqlitePool, policy: &PolicySpec) -> Result<(), sqlx::Error> {
    let config_json =
        serde_json::to_string(&policy.default_config).unwrap_or_else(|_| "{}".to_string());
    let supported_stages_json = serde_json::to_string(&policy.supported_stages)
        .unwrap_or_else(|_| r#"["pre_route","pre_upstream","post_response"]"#.to_string());
    let config_schema_json = serde_json::to_string(&policy.config_schema)
        .unwrap_or_else(|_| r#"{"type":"object"}"#.to_string());
    let default_config_json =
        serde_json::to_string(&policy.default_config).unwrap_or_else(|_| "{}".to_string());
    let now = current_ts();
    sqlx::query(
        r#"
        INSERT INTO policies (
            id,
            version,
            wasm_uri,
            sha256,
            config_json,
            supported_stages_json,
            config_schema_json,
            default_config_json,
            created_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
        "#,
    )
    .bind(&policy.id)
    .bind(&policy.version)
    .bind(&policy.wasm_uri)
    .bind(&policy.sha256)
    .bind(config_json)
    .bind(supported_stages_json)
    .bind(config_schema_json)
    .bind(default_config_json)
    .bind(now)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn list_policies(pool: &SqlitePool) -> Result<Vec<PolicySpec>, sqlx::Error> {
    let rows = sqlx::query(
        r#"
        SELECT id, version, wasm_uri, sha256, config_json, supported_stages_json, config_schema_json, default_config_json
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
            SELECT id, version, wasm_uri, sha256, config_json, supported_stages_json, config_schema_json, default_config_json
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
            SELECT id, version, wasm_uri, sha256, config_json, supported_stages_json, config_schema_json, default_config_json
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

pub async fn get_policy_version(
    pool: &SqlitePool,
    id: &str,
    version: &str,
) -> Result<Option<PolicySpec>, sqlx::Error> {
    let row = sqlx::query(
        r#"
        SELECT id, version, wasm_uri, sha256, config_json, supported_stages_json, config_schema_json, default_config_json
        FROM policies
        WHERE id = ?1 AND version = ?2
        "#,
    )
    .bind(id)
    .bind(version)
    .fetch_optional(pool)
    .await?;

    row.map(row_to_policy).transpose()
}

fn row_to_policy(row: sqlx::sqlite::SqliteRow) -> Result<PolicySpec, sqlx::Error> {
    use sqlx::Row;

    let id: String = row.try_get("id")?;
    let version: String = row.try_get("version")?;
    let wasm_uri: String = row.try_get("wasm_uri")?;
    let sha256: String = row.try_get("sha256")?;
    let config_json: String = row.try_get("config_json")?;
    let supported_stages_json: String = row
        .try_get("supported_stages_json")
        .unwrap_or_else(|_| r#"["pre_route","pre_upstream","post_response"]"#.to_string());
    let config_schema_json: String = row
        .try_get("config_schema_json")
        .unwrap_or_else(|_| r#"{"type":"object"}"#.to_string());
    let default_config_json: String = row
        .try_get("default_config_json")
        .unwrap_or_else(|_| config_json.clone());

    let supported_stages = serde_json::from_str(&supported_stages_json).unwrap_or_else(|_| {
        vec![
            "pre_route".to_string(),
            "pre_upstream".to_string(),
            "post_response".to_string(),
        ]
    });
    let config_schema = serde_json::from_str(&config_schema_json)
        .unwrap_or_else(|_| serde_json::json!({"type": "object"}));
    let default_config =
        serde_json::from_str(&default_config_json).unwrap_or(serde_json::json!({}));

    Ok(PolicySpec {
        id,
        version,
        wasm_uri,
        sha256,
        supported_stages,
        config_schema,
        default_config,
    })
}
