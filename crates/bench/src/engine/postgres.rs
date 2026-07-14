use std::path::Path;
use std::str::FromStr;
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use postgres::config::Host;
use postgres::types::{ToSql, Type};
use postgres::{Client, Config, NoTls, Row};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::config::{DurabilityKind, RunSpec};
use crate::engine::{BenchConn, BenchEngine, CellValue, EngineSnapshot, seeded_blob};

const CI_SERVICE_ENV: &str = "REDLINEDB_BENCH_POSTGRES_CI_SERVICE";
const CI_SERVICE_HOST: &str = "postgres-cert";
const DIND_SERVICE_ENV: &str = "REDLINEDB_BENCH_POSTGRES_DOCKER_DAEMON_SERVICE";
const DIND_SERVICE_HOST: &str = "docker";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PostgresEndpoint {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PostgresLiveIdentity {
    pub database: String,
    pub system_identifier: String,
    pub postmaster_started_at: String,
    pub server_address: String,
    pub server_port: u16,
    pub data_directory: String,
}

/// PostgreSQL adapter used only by the explicit three-engine interaction/volume certificate.
/// The regular SQLite-compatibility matrix remains exactly Redline/SQLite and keeps its stable
/// `EngineKind` serialization.
pub(crate) struct PostgresEngine {
    dsn: String,
    schema: String,
    durability: DurabilityKind,
    server_identity: serde_json::Value,
}

impl PostgresEngine {
    pub(crate) fn create(spec: &RunSpec, db_dir: &Path, dsn: &str) -> Result<Self> {
        validate_local_dedicated_dsn(dsn)?;
        let schema = schema_name(db_dir);
        let mut client = Client::connect(dsn, NoTls).context("connect to PostgreSQL")?;
        let server_identity = validate_dedicated_server(&mut client)?;
        client
            .batch_execute(&format!(
                "DROP SCHEMA IF EXISTS {schema} CASCADE; CREATE SCHEMA {schema}"
            ))
            .context("create isolated PostgreSQL benchmark schema")?;
        Ok(Self {
            dsn: dsn.to_owned(),
            schema,
            durability: spec.durability,
            server_identity,
        })
    }

    pub(crate) fn reopen(spec: &RunSpec, db_dir: &Path, dsn: &str) -> Result<Self> {
        validate_local_dedicated_dsn(dsn)?;
        let schema = schema_name(db_dir);
        let mut client = Client::connect(dsn, NoTls).context("reconnect to PostgreSQL")?;
        let server_identity = validate_dedicated_server(&mut client)?;
        let exists = client
            .query_one(
                "SELECT EXISTS(SELECT 1 FROM pg_namespace WHERE nspname = $1)",
                &[&schema],
            )?
            .get::<_, bool>(0);
        if !exists {
            bail!("PostgreSQL benchmark schema {schema} disappeared before reopen verification");
        }
        Ok(Self {
            dsn: dsn.to_owned(),
            schema,
            durability: spec.durability,
            server_identity,
        })
    }

    pub(crate) fn cleanup_for_path(dsn: &str, db_dir: &Path) -> Result<()> {
        cleanup_schema(dsn, &schema_name(db_dir))
    }

    pub(crate) fn benchmark_schema_count(dsn: &str) -> Result<u64> {
        validate_local_dedicated_dsn(dsn)?;
        let mut client = Client::connect(dsn, NoTls).context("connect for PG cleanup proof")?;
        let _ = validate_dedicated_server(&mut client)?;
        let count = client
            .query_one(
                "SELECT COUNT(*)::bigint FROM pg_namespace \
                 WHERE nspname LIKE 'redline_interaction_%'",
                &[],
            )?
            .get::<_, i64>(0);
        u64::try_from(count).context("PostgreSQL benchmark schema count was negative")
    }

    pub(crate) fn endpoint_scope(dsn: &str) -> Result<&'static str> {
        validate_local_dedicated_dsn(dsn)?;
        if std::env::var(CI_SERVICE_ENV).as_deref() == Ok("1") {
            Ok("ci_service")
        } else if std::env::var(DIND_SERVICE_ENV).as_deref() == Ok("1") {
            Ok("docker_daemon_service")
        } else {
            Ok("loopback")
        }
    }

    pub(crate) fn endpoint(dsn: &str) -> Result<PostgresEndpoint> {
        let config = Config::from_str(dsn).context("parse PostgreSQL benchmark DSN")?;
        if config.get_hosts().len() != 1 {
            bail!("PostgreSQL benchmark DSN must resolve exactly one host");
        }
        let host = match &config.get_hosts()[0] {
            Host::Tcp(host) => host.clone(),
            #[cfg(unix)]
            Host::Unix(_) => bail!("Docker-bound certificate requires a TCP PostgreSQL endpoint"),
        };
        let port = config.get_ports().first().copied().unwrap_or(5432);
        Ok(PostgresEndpoint { host, port })
    }

    pub(crate) fn live_identity(dsn: &str) -> Result<PostgresLiveIdentity> {
        validate_local_dedicated_dsn(dsn)?;
        let mut client = Client::connect(dsn, NoTls).context("connect for live PG identity")?;
        let _ = validate_dedicated_server(&mut client)?;
        let row = client.query_one(
            "SELECT current_database(), system_identifier::text, \
                    pg_postmaster_start_time()::text, \
                    COALESCE(inet_server_addr()::text, 'local-unix'), \
                    COALESCE(inet_server_port(), 0)::int, current_setting('data_directory') \
             FROM pg_control_system()",
            &[],
        )?;
        let server_port = row.get::<_, i32>(4);
        Ok(PostgresLiveIdentity {
            database: row.get(0),
            system_identifier: row.get(1),
            postmaster_started_at: row.get(2),
            server_address: row.get(3),
            server_port: u16::try_from(server_port)
                .context("PostgreSQL reported an invalid server port")?,
            data_directory: row.get(5),
        })
    }

    pub(crate) fn server_version(&self) -> Result<String> {
        let mut client = self.open_client()?;
        Ok(client.query_one("SHOW server_version", &[])?.get(0))
    }

    fn open_client(&self) -> Result<Client> {
        let mut client = Client::connect(&self.dsn, NoTls).context("connect to PostgreSQL")?;
        let synchronous_commit = match self.durability {
            DurabilityKind::Strict | DurabilityKind::Normal => "on",
            DurabilityKind::Unsafe => "off",
        };
        client.batch_execute(&format!(
            "SET search_path TO {}; SET synchronous_commit TO {synchronous_commit}; \
             SET lock_timeout TO '5s'; SET statement_timeout TO '30s'; SET jit TO off",
            self.schema
        ))?;
        Ok(client)
    }
}

impl BenchEngine for PostgresEngine {
    fn connect(&self, _worker_id: usize) -> Result<Box<dyn BenchConn>> {
        Ok(Box::new(PostgresConn {
            client: self.open_client()?,
        }))
    }

    fn setup_schema(&self) -> Result<()> {
        let mut client = self.open_client()?;
        client.batch_execute(
            "CREATE TABLE IF NOT EXISTS kv(\
                 k BIGINT PRIMARY KEY, tenant BIGINT NOT NULL, v BYTEA NOT NULL, version BIGINT NOT NULL\
             );\
             CREATE INDEX IF NOT EXISTS kv_tenant_idx ON kv(tenant)",
        )?;
        Ok(())
    }

    fn seed_kv(&self, rows: usize) -> Result<()> {
        let mut client = self.open_client()?;
        let mut tx = client.transaction()?;
        tx.execute("DELETE FROM kv", &[])?;
        let statement =
            tx.prepare("INSERT INTO kv(k, tenant, v, version) VALUES ($1, $2, $3, $4)")?;
        for index in 0..rows {
            tx.execute(
                &statement,
                &[
                    &(index as i64),
                    &((index % 32) as i64),
                    &seeded_blob(index),
                    &1_i64,
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    fn checkpoint(&self) -> Result<()> {
        self.open_client()?
            .batch_execute("CHECKPOINT")
            .context("PostgreSQL CHECKPOINT requires a benchmark role with checkpoint privilege")
    }

    fn snapshot(&self) -> Result<EngineSnapshot> {
        let mut client = self.open_client()?;
        // The certificate owns a dedicated PostgreSQL cluster. Account for its full default
        // tablespace, not only the active schema/database, so catalogs, indexes, prior-run
        // residue, and temporary relation growth all consume the same absolute safety budget.
        let data_bytes = client
            .query_one("SELECT pg_tablespace_size('pg_default')::bigint", &[])?
            .get::<_, i64>(0);
        let data_bytes =
            u64::try_from(data_bytes).context("PostgreSQL default tablespace size was negative")?;
        let wal_bytes = client
            .query_one(
                "SELECT COALESCE(SUM(size), 0)::bigint FROM pg_ls_waldir()",
                &[],
            )?
            .get::<_, i64>(0);
        let wal_bytes =
            u64::try_from(wal_bytes).context("PostgreSQL WAL directory size was negative")?;
        let version: String = client.query_one("SHOW server_version", &[])?.get(0);
        let synchronous_commit: String = client.query_one("SHOW synchronous_commit", &[])?.get(0);
        let full_page_writes: String = client.query_one("SHOW full_page_writes", &[])?.get(0);
        Ok(EngineSnapshot {
            data_bytes,
            wal_bytes,
            engine_stats: json!({
                "server_version": version,
                "synchronous_commit": synchronous_commit,
                "full_page_writes": full_page_writes,
                "schema": self.schema,
                "dedicated_server": self.server_identity,
            }),
            fsyncs_issued: None,
            fdatasyncs_issued: None,
            pwrites_issued: None,
        })
    }

    fn checksum(&self) -> Result<crate::report::Checksum> {
        let mut conn = PostgresConn {
            client: self.open_client()?,
        };
        crate::engine::kv_checksum(&mut conn)
    }
}

struct PostgresConn {
    client: Client,
}

impl BenchConn for PostgresConn {
    fn execute(&mut self, sql: &str, params: &[CellValue]) -> Result<u64> {
        let sql = postgres_placeholders(sql);
        let params = postgres_params(params);
        let refs = postgres_param_refs(&params);
        Ok(self.client.execute(&sql, &refs)?)
    }

    fn query_row(&mut self, sql: &str, params: &[CellValue]) -> Result<Vec<CellValue>> {
        Ok(self
            .query_all(sql, params)?
            .into_iter()
            .next()
            .unwrap_or_default())
    }

    fn query_all(&mut self, sql: &str, params: &[CellValue]) -> Result<Vec<Vec<CellValue>>> {
        let sql = postgres_placeholders(sql);
        let params = postgres_params(params);
        let refs = postgres_param_refs(&params);
        self.client
            .query(&sql, &refs)?
            .iter()
            .map(postgres_row)
            .collect()
    }

    fn begin_immediate(&mut self) -> Result<()> {
        self.client.batch_execute("BEGIN")?;
        Ok(())
    }

    fn commit(&mut self) -> Result<()> {
        self.client.batch_execute("COMMIT")?;
        Ok(())
    }

    fn rollback(&mut self) -> Result<()> {
        self.client.batch_execute("ROLLBACK")?;
        Ok(())
    }

    fn set_busy_timeout(&mut self, timeout: Duration) -> Result<()> {
        let millis = timeout.as_millis().min(u128::from(u64::MAX)) as u64;
        self.client
            .batch_execute(&format!("SET lock_timeout TO '{millis}ms'"))?;
        Ok(())
    }
}

fn postgres_params(values: &[CellValue]) -> Vec<Box<dyn ToSql + Sync>> {
    values
        .iter()
        .map(|value| -> Box<dyn ToSql + Sync> {
            match value {
                CellValue::Null => Box::new(None::<i64>),
                CellValue::Integer(value) => Box::new(*value),
                CellValue::Real(value) => Box::new(*value),
                CellValue::Text(value) => Box::new(value.clone()),
                CellValue::Blob(value) => Box::new(value.clone()),
            }
        })
        .collect()
}

fn postgres_param_refs(values: &[Box<dyn ToSql + Sync>]) -> Vec<&(dyn ToSql + Sync)> {
    values.iter().map(Box::as_ref).collect()
}

fn postgres_row(row: &Row) -> Result<Vec<CellValue>> {
    row.columns()
        .iter()
        .enumerate()
        .map(|(index, column)| postgres_cell(row, index, column.type_()))
        .collect()
}

fn postgres_cell(row: &Row, index: usize, kind: &Type) -> Result<CellValue> {
    if *kind == Type::INT2 {
        return optional_cell(row.try_get::<_, Option<i16>>(index)?, |value| {
            CellValue::Integer(i64::from(value))
        });
    }
    if *kind == Type::INT4 {
        return optional_cell(row.try_get::<_, Option<i32>>(index)?, |value| {
            CellValue::Integer(i64::from(value))
        });
    }
    if *kind == Type::INT8 {
        return optional_cell(row.try_get::<_, Option<i64>>(index)?, CellValue::Integer);
    }
    if *kind == Type::FLOAT4 {
        return optional_cell(row.try_get::<_, Option<f32>>(index)?, |value| {
            CellValue::Real(f64::from(value))
        });
    }
    if *kind == Type::FLOAT8 {
        return optional_cell(row.try_get::<_, Option<f64>>(index)?, CellValue::Real);
    }
    if *kind == Type::BYTEA {
        return optional_cell(row.try_get::<_, Option<Vec<u8>>>(index)?, CellValue::Blob);
    }
    if *kind == Type::BOOL {
        return optional_cell(row.try_get::<_, Option<bool>>(index)?, |value| {
            CellValue::Integer(i64::from(value))
        });
    }
    if matches!(
        *kind,
        Type::TEXT | Type::VARCHAR | Type::BPCHAR | Type::NAME | Type::JSON | Type::JSONB
    ) {
        return optional_cell(row.try_get::<_, Option<String>>(index)?, CellValue::Text);
    }
    Err(anyhow!(
        "unsupported PostgreSQL benchmark result type {kind}"
    ))
}

fn optional_cell<T>(value: Option<T>, map: impl FnOnce(T) -> CellValue) -> Result<CellValue> {
    Ok(value.map(map).unwrap_or(CellValue::Null))
}

fn postgres_placeholders(sql: &str) -> String {
    let mut converted = sql.to_owned();
    for index in (1..=64).rev() {
        converted = converted.replace(&format!("?{index}"), &format!("${index}"));
    }
    converted
}

fn schema_name(path: &Path) -> String {
    let digest = Sha256::digest(path.as_os_str().as_encoded_bytes());
    let suffix = digest[..10]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("redline_interaction_{suffix}")
}

fn cleanup_schema(dsn: &str, schema: &str) -> Result<()> {
    let mut client = Client::connect(dsn, NoTls).context("connect for PG cleanup")?;
    client
        .batch_execute(&format!("DROP SCHEMA IF EXISTS {schema} CASCADE"))
        .context("drop isolated PostgreSQL benchmark schema")
}

fn validate_local_dedicated_dsn(dsn: &str) -> Result<()> {
    if std::env::var("REDLINEDB_BENCH_POSTGRES_ISOLATED").as_deref() != Ok("1") {
        bail!(
            "REDLINEDB_BENCH_POSTGRES_ISOLATED=1 is required; the certificate may only use a disposable dedicated PostgreSQL instance"
        );
    }
    let config = Config::from_str(dsn).context("parse PostgreSQL benchmark DSN")?;
    let ci_service = std::env::var(CI_SERVICE_ENV).as_deref() == Ok("1");
    let dind_service = std::env::var(DIND_SERVICE_ENV).as_deref() == Ok("1");
    if ci_service && std::env::var("CI").as_deref() != Ok("true") {
        bail!("{CI_SERVICE_ENV}=1 is accepted only inside CI");
    }
    if dind_service && std::env::var("CI").as_deref() != Ok("true") {
        bail!("{DIND_SERVICE_ENV}=1 is accepted only inside CI");
    }
    if ci_service && dind_service {
        bail!("PostgreSQL CI service and Docker-daemon service modes are mutually exclusive");
    }
    if config.get_hosts().is_empty() {
        bail!("PostgreSQL benchmark DSN must name a local host or Unix socket");
    }
    for host in config.get_hosts() {
        match host {
            Host::Tcp(host) if host == "localhost" => {}
            Host::Tcp(host) if ci_service && host == CI_SERVICE_HOST => {}
            Host::Tcp(host) if dind_service && host == DIND_SERVICE_HOST => {}
            Host::Tcp(host) => {
                let address = host.parse::<std::net::IpAddr>().with_context(
                    || "PostgreSQL benchmark host must be localhost or a loopback IP",
                )?;
                if !address.is_loopback() {
                    bail!("PostgreSQL benchmark host must be local loopback");
                }
            }
            #[cfg(unix)]
            Host::Unix(_) => {}
        }
    }
    if config
        .get_hostaddrs()
        .iter()
        .any(|address| !address.is_loopback())
    {
        bail!("PostgreSQL benchmark hostaddr must be local loopback");
    }
    Ok(())
}

fn validate_dedicated_server(client: &mut Client) -> Result<serde_json::Value> {
    let fsync: String = client.query_one("SHOW fsync", &[])?.get(0);
    let synchronous_commit: String = client.query_one("SHOW synchronous_commit", &[])?.get(0);
    let full_page_writes: String = client.query_one("SHOW full_page_writes", &[])?.get(0);
    let data_checksums: String = client.query_one("SHOW data_checksums", &[])?.get(0);
    if fsync != "on"
        || synchronous_commit != "on"
        || full_page_writes != "on"
        || data_checksums != "on"
    {
        bail!(
            "PostgreSQL durability gate requires fsync=on, synchronous_commit=on, full_page_writes=on, and data_checksums=on"
        );
    }
    let other_clients = client
        .query_one(
            "SELECT COUNT(*)::bigint FROM pg_stat_activity \
             WHERE pid <> pg_backend_pid() AND backend_type = 'client backend'",
            &[],
        )?
        .get::<_, i64>(0);
    if other_clients != 0 {
        bail!("PostgreSQL benchmark instance is not dedicated: {other_clients} other clients");
    }
    let database: String = client.query_one("SELECT current_database()", &[])?.get(0);
    let system_identifier: String = client
        .query_one(
            "SELECT system_identifier::text FROM pg_control_system()",
            &[],
        )?
        .get(0);
    let postmaster_started_at: String = client
        .query_one("SELECT pg_postmaster_start_time()::text", &[])?
        .get(0);
    let server_address: String = client
        .query_one(
            "SELECT COALESCE(inet_server_addr()::text, 'local-unix')",
            &[],
        )?
        .get(0);
    Ok(json!({
        "database": database,
        "system_identifier": system_identifier,
        "postmaster_started_at": postmaster_started_at,
        "server_address": server_address,
        "fsync": fsync,
        "synchronous_commit": synchronous_commit,
        "full_page_writes": full_page_writes,
        "data_checksums": data_checksums,
        "other_client_backends_at_preflight": other_clients,
    }))
}

#[cfg(test)]
mod tests {
    use super::{postgres_placeholders, schema_name};
    use std::path::Path;

    #[test]
    fn placeholders_convert_without_corrupting_double_digits() {
        assert_eq!(
            postgres_placeholders("SELECT ?1, ?10, ?2"),
            "SELECT $1, $10, $2"
        );
    }

    #[test]
    fn schema_name_is_stable_and_identifier_safe() {
        let name = schema_name(Path::new("/tmp/example"));
        assert!(name.starts_with("redline_interaction_"));
        assert!(
            name.bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
        );
        assert_eq!(name, schema_name(Path::new("/tmp/example")));
    }
}
