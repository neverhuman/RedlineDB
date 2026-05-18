use std::env;
use std::path::PathBuf;
use std::process::exit;

use redlinedb::{
    ArchiveMode, BackupOptions, Database, PhysicalBackupOptions, RecoveryTarget, RestoreOptions,
    Step, ValueRef,
};
use serde_json::json;

fn main() {
    if let Err(message) = run() {
        eprintln!("{message}");
        exit(1);
    }
}

fn run() -> Result<(), String> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if args.is_empty() || args.iter().any(|arg| arg == "--help" || arg == "-h") {
        print_help();
        return Ok(());
    }

    match args[0].as_str() {
        "backup" => run_backup(&args),
        "restore" => run_restore(&args),
        "archive-check" => run_archive_check(&args),
        "replication-slot" => run_replication_slot(&args),
        "stream-wal" => run_stream_wal(&args),
        "stream-logical" => run_stream_logical(&args),
        "stats" => run_stats(&args),
        _ if args.len() >= 2 => {
            let db = Database::open(&args[0]).map_err(|err| err.to_string())?;
            let sql = args[1..].join(" ");
            run_query(db, &sql)
        }
        _ => Err("usage: redlinedb DB SQL".to_owned()),
    }
}

fn run_backup(args: &[String]) -> Result<(), String> {
    if args.len() < 3 {
        return Err("usage: redlinedb backup SRC DST [--logical|--physical]".to_owned());
    }
    let src = PathBuf::from(&args[1]);
    let dst = PathBuf::from(&args[2]);
    let logical = args.iter().any(|arg| arg == "--logical");
    let db = Database::open(&src).map_err(|err| err.to_string())?;
    if logical {
        let _ = db
            .backup_to_path(dst, BackupOptions::default())
            .map_err(|err| err.to_string())?;
    } else {
        let _ = db
            .backup_physical_to_path(
                dst,
                PhysicalBackupOptions {
                    include_wal: true,
                    archive_mode: ArchiveMode::Off,
                },
            )
            .map_err(|err| err.to_string())?;
    }
    Ok(())
}

fn run_restore(args: &[String]) -> Result<(), String> {
    if args.len() < 3 {
        return Err(
            "usage: redlinedb restore BACKUP DST [--target-lsn N|--target-csn N|--latest]"
                .to_owned(),
        );
    }
    let src = PathBuf::from(&args[1]);
    let dst = PathBuf::from(&args[2]);
    let mut target = RecoveryTarget::Latest;
    let mut index = 3;
    while index < args.len() {
        match args[index].as_str() {
            "--latest" => target = RecoveryTarget::Latest,
            "--target-lsn" if index + 1 < args.len() => {
                target = RecoveryTarget::Lsn(redlinedb::Lsn(
                    args[index + 1]
                        .parse::<u64>()
                        .map_err(|err| err.to_string())?,
                ));
                index += 1;
            }
            "--target-csn" if index + 1 < args.len() => {
                target = RecoveryTarget::Csn(redlinedb::Csn(
                    args[index + 1]
                        .parse::<u64>()
                        .map_err(|err| err.to_string())?,
                ));
                index += 1;
            }
            other => return Err(format!("unknown restore flag: {other}")),
        }
        index += 1;
    }
    let _ = Database::restore_from_backup(
        src,
        dst,
        RestoreOptions {
            target,
            preserve_timeline: false,
        },
    )
    .map_err(|err| err.to_string())?;
    Ok(())
}

fn run_archive_check(args: &[String]) -> Result<(), String> {
    if args.len() < 2 {
        return Err("usage: redlinedb archive-check DB [--json]".to_owned());
    }
    let db = Database::open(&args[1]).map_err(|err| err.to_string())?;
    let stats = db.archive_stats().map_err(|err| err.to_string())?;
    if args.iter().any(|arg| arg == "--json") {
        println!(
            "{}",
            serde_json::to_string(&stats).map_err(|err| err.to_string())?
        );
    } else {
        println!("archive_mode={:?}", stats.archive_mode);
        println!("pending_segments={}", stats.pending_segments);
        println!("archived_segments={}", stats.archived_segments);
        println!("failed_segments={}", stats.failed_segments);
        println!("last_archived_lsn={}", stats.last_archived_lsn);
        println!("archived_bytes={}", stats.archived_bytes);
    }
    Ok(())
}

fn run_replication_slot(args: &[String]) -> Result<(), String> {
    if args.len() < 4 {
        return Err(
            "usage: redlinedb replication-slot create|drop|list DB NAME [--physical|--logical] [--json]".to_owned(),
        );
    }

    match args[1].as_str() {
        "create" => {
            let db = Database::open(&args[2]).map_err(|err| err.to_string())?;
            let name = &args[3];
            let slot = if args.iter().any(|arg| arg == "--logical") {
                db.create_logical_slot(name)
                    .map_err(|err| err.to_string())?
            } else {
                let _ = args.iter().any(|arg| arg == "--physical");
                db.create_physical_slot(name)
                    .map_err(|err| err.to_string())?
            };
            println!(
                "{}",
                serde_json::to_string(&slot).map_err(|err| err.to_string())?
            );
            Ok(())
        }
        "drop" => {
            let db = Database::open(&args[2]).map_err(|err| err.to_string())?;
            db.drop_replication_slot(&args[3])
                .map_err(|err| err.to_string())?;
            Ok(())
        }
        "list" => {
            let db = Database::open(&args[2]).map_err(|err| err.to_string())?;
            let slots = db.replication_slots().map_err(|err| err.to_string())?;
            if args.iter().any(|arg| arg == "--json") {
                println!(
                    "{}",
                    serde_json::to_string(&slots).map_err(|err| err.to_string())?
                );
            } else {
                for slot in slots {
                    println!(
                        "{}\t{:?}\trestart_lsn={}\trestart_csn={}\tactive={}",
                        slot.name, slot.kind, slot.restart_lsn, slot.restart_csn, slot.active
                    );
                }
            }
            Ok(())
        }
        other => Err(format!("unknown replication-slot subcommand: {other}")),
    }
}

fn run_stream_wal(args: &[String]) -> Result<(), String> {
    if args.len() != 3 {
        return Err("usage: redlinedb stream-wal DB SLOT".to_owned());
    }
    let db = Database::open(&args[1]).map_err(|err| err.to_string())?;
    let slots = db.replication_slots().map_err(|err| err.to_string())?;
    let slot = slots
        .into_iter()
        .find(|slot| slot.name == args[2])
        .ok_or_else(|| "replication slot not found".to_owned())?;
    let archive = db.archive_stats().map_err(|err| err.to_string())?;
    println!(
        "{}",
        json!({
            "slot": slot.name,
            "kind": format!("{:?}", slot.kind),
            "restart_lsn": slot.restart_lsn,
            "restart_csn": slot.restart_csn,
            "archive": archive,
        })
    );
    Ok(())
}

fn run_stream_logical(args: &[String]) -> Result<(), String> {
    if args.len() != 3 && args.len() != 4 {
        return Err("usage: redlinedb stream-logical DB SLOT [--ndjson]".to_owned());
    }
    let _ndjson = args.iter().any(|arg| arg == "--ndjson");
    let db = Database::open(&args[1]).map_err(|err| err.to_string())?;
    let slots = db.replication_slots().map_err(|err| err.to_string())?;
    let slot = slots
        .into_iter()
        .find(|slot| slot.name == args[2])
        .ok_or_else(|| "replication slot not found".to_owned())?;
    let payload = json!({
        "slot": slot.name,
        "kind": format!("{:?}", slot.kind),
        "restart_csn": slot.restart_csn,
        "confirmed_flush_csn": slot.confirmed_flush_csn,
        "active": slot.active,
    });
    println!("{}", payload);
    Ok(())
}

fn run_stats(args: &[String]) -> Result<(), String> {
    if args.len() < 2 {
        return Err("usage: redlinedb stats DB [--json]".to_owned());
    }
    let json_output = args.iter().any(|arg| arg == "--json");
    let db = Database::open(&args[1]).map_err(|err| err.to_string())?;
    let stats = db.stats().map_err(|err| err.to_string())?;
    if json_output {
        println!(
            "{{\"schema_epoch\":{},\"resident_heap_pages\":{},\"wal_written_lsn\":{},\"wal_durable_lsn\":{}}}",
            stats.schema_epoch,
            stats.resident_heap_pages,
            stats.wal_written_lsn,
            stats.wal_durable_lsn
        );
    } else {
        println!("schema_epoch={}", stats.schema_epoch);
        println!("resident_heap_pages={}", stats.resident_heap_pages);
        println!("wal_written_lsn={}", stats.wal_written_lsn);
        println!("wal_durable_lsn={}", stats.wal_durable_lsn);
    }
    Ok(())
}

fn run_query(db: Database, sql: &str) -> Result<(), String> {
    let mut conn = db.connect().map_err(|err| err.to_string())?;
    let mut stmt = conn.prepare(sql).map_err(|err| err.to_string())?;
    let column_count = stmt.column_count();
    while let Step::Row(row) = stmt.step().map_err(|err| err.to_string())? {
        let mut first = true;
        for index in 0..column_count {
            if !first {
                print!("\t");
            }
            first = false;
            match row.get_ref(index).map_err(|err| err.to_string())? {
                ValueRef::Null => print!("NULL"),
                ValueRef::Integer(value) => print!("{value}"),
                ValueRef::Real(value) => print!("{value}"),
                ValueRef::Text(value) => print!("{value}"),
                ValueRef::Blob(value) => print!("<blob:{}>", value.len()),
            }
        }
        println!();
    }
    Ok(())
}

fn print_help() {
    println!("redlinedb backup SRC DST [--logical|--physical]");
    println!("redlinedb restore BACKUP DST [--target-lsn N|--target-csn N|--latest]");
    println!("redlinedb archive-check DB [--json]");
    println!("redlinedb replication-slot create|drop|list DB NAME [--physical|--logical] [--json]");
    println!("redlinedb stream-wal DB SLOT");
    println!("redlinedb stream-logical DB SLOT [--ndjson]");
    println!("redlinedb stats DB [--json]");
    println!("redlinedb DB SQL");
}
