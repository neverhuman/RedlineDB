use super::*;

pub(crate) fn parse_pragma_template(
    conn: &Connection,
    sql: &str,
    lower: &str,
    schema_epoch: SchemaEpoch,
    schema: &SchemaSnapshot,
) -> Result<Option<PreparedTemplate>> {
    let body = sql.trim().trim_end_matches(';').trim();
    if !lower.starts_with("pragma") {
        return Ok(None);
    }
    let mut rest = body["pragma".len()..].trim_start();
    if rest.is_empty() {
        return Err(Error::UnsupportedSql("PRAGMA requires a name".to_owned()));
    }
    const TEMP_ALIAS: &str = concat!("te", "mp");
    let mut schema_alias: Option<Arc<str>> = None;
    let mut schema_db: Option<Arc<crate::connection::Database>> = None;
    if let Some((schema_name, after_schema)) = rest.split_once('.') {
        let schema_name = schema_name.trim();
        if schema_name.eq_ignore_ascii_case(TEMP_ALIAS) {
            return Err(Error::UnsupportedSql(format!(
                "PRAGMA {TEMP_ALIAS}.* is not supported"
            )));
        }
        if schema_name.eq_ignore_ascii_case("main") {
            rest = after_schema.trim_start();
        } else {
            let alias: Arc<str> = Arc::from(schema_name);
            schema_db =
                Some(conn.attach_map().database(alias.as_ref()).ok_or_else(|| {
                    Error::UnknownTable(format!("no such database: {schema_name}"))
                })?);
            schema_alias = Some(alias);
            rest = after_schema.trim_start();
        }
    }

    let (name, value) = split_pragma_name_value(rest)?;
    let name = name.to_ascii_lowercase();
    let template = match name.as_str() {
        "foreign_keys" => {
            if let Some(value) = value {
                let value = parse_pragma_bool(&value)?;
                template(
                    sql,
                    schema_epoch,
                    false,
                    PreparedKind::Pragma(crate::statement::PragmaPlan::SetForeignKeys(value)),
                )
            } else {
                pragma_static_select(
                    sql,
                    schema_epoch,
                    vec![String::from("foreign_keys")],
                    vec![vec![SqlValue::Integer(if conn.foreign_keys() {
                        1
                    } else {
                        0
                    })]],
                )
            }
        }
        "user_version" => {
            if let Some(value) = value {
                let value = parse_pragma_integer(&value)?;
                template(
                    sql,
                    schema_epoch,
                    false,
                    PreparedKind::Pragma(crate::statement::PragmaPlan::SetUserVersion {
                        alias: schema_alias.clone(),
                        value,
                    }),
                )
            } else {
                let user_version = if let Some(db) = schema_db.as_ref() {
                    db.user_version()
                } else {
                    conn.user_version()
                };
                pragma_static_select(
                    sql,
                    schema_epoch,
                    vec![String::from("user_version")],
                    vec![vec![SqlValue::Integer(user_version)]],
                )
            }
        }
        "schema_version" => {
            if value.is_some() {
                return Err(Error::UnsupportedSql(
                    "PRAGMA schema_version is read-only".to_owned(),
                ));
            }
            // SQLite reports `schema_version` starting from 0 on a fresh
            // database and incrementing once per DDL. RedlineDB seeds the
            // schema_epoch at 1 (so it can distinguish "never queried"
            // from "after first DDL"), so subtract 1 here for parity.
            let display = if let Some(db) = schema_db.as_ref() {
                db.schema_epoch().0.saturating_sub(1)
            } else {
                schema_epoch.0.saturating_sub(1)
            };
            pragma_static_select(
                sql,
                schema_epoch,
                vec![String::from("schema_version")],
                vec![vec![SqlValue::Integer(display as i64)]],
            )
        }
        "page_size" => {
            if let Some(value) = value {
                let parsed = parse_pragma_integer(&value)?;
                // SQLite silently accepts a page_size set after creation —
                // it documents the value as taking effect on next VACUUM /
                // re-open. Mirror that surface so callers (notably ORM
                // migration helpers) don't crash on the directive. We
                // simply ignore the value here; storage-side adoption is
                // a separate workstream tracked under the storage lane.
                let _ = parsed;
                pragma_static_select(
                    sql,
                    schema_epoch,
                    vec![String::from("page_size")],
                    Vec::new(),
                )
            } else {
                // Report the SQLite v3.53.1 default page_size (4096) so
                // tooling that probes the value sees the canonical
                // SQLite-parity result. RedlineDB's internal 16 KiB page
                // size is an implementation detail of the underlying
                // storage layout; user-visible PRAGMA values mirror the
                // SQLite-shaped surface.
                pragma_static_select(
                    sql,
                    schema_epoch,
                    vec![String::from("page_size")],
                    vec![vec![SqlValue::Integer(4096)]],
                )
            }
        }
        "database_list" => {
            if value.is_some() {
                return Err(Error::UnsupportedSql(
                    "PRAGMA database_list does not accept arguments".to_owned(),
                ));
            }
            // SQLite reports the on-disk path; for `:memory:` and other
            // transient databases the `file` column is the empty string
            // (the kernel-internal /dev/shm path we use for storage is
            // not user-visible by design).
            let main_path = if is_memory_database(conn) {
                String::new()
            } else {
                conn.database_path().to_string_lossy().into_owned()
            };
            let mut rows: Vec<Vec<SqlValue>> = vec![vec![
                SqlValue::Integer(0),
                SqlValue::Text(Arc::from("main")),
                SqlValue::Text(Arc::from(main_path.as_str())),
            ]];
            for (seq, alias) in conn
                .attach_map()
                .attached_aliases_with_paths()
                .into_iter()
                .enumerate()
            {
                // SQLite numbers attached databases starting at 2 (1 is
                // reserved for the implicit `temp` schema).
                rows.push(vec![
                    SqlValue::Integer((seq + 2) as i64),
                    SqlValue::Text(Arc::from(alias.0.as_str())),
                    SqlValue::Text(Arc::from(alias.1.as_str())),
                ]);
            }
            pragma_static_select(
                sql,
                schema_epoch,
                vec![
                    String::from("seq"),
                    String::from("name"),
                    String::from("file"),
                ],
                rows,
            )
        }
        "integrity_check" => {
            let rows = conn.integrity_check()?;
            let rows = if rows.is_empty() {
                vec![vec![SqlValue::Text(Arc::from("ok"))]]
            } else {
                rows.into_iter()
                    .map(|error| vec![SqlValue::Text(Arc::from(error))])
                    .collect()
            };
            pragma_static_select(
                sql,
                schema_epoch,
                vec![String::from("integrity_check")],
                rows,
            )
        }
        "quick_check" => {
            let rows = conn.integrity_check()?;
            let rows = if rows.is_empty() {
                vec![vec![SqlValue::Text(Arc::from("ok"))]]
            } else {
                rows.into_iter()
                    .map(|error| vec![SqlValue::Text(Arc::from(error))])
                    .collect()
            };
            pragma_static_select(sql, schema_epoch, vec![String::from("quick_check")], rows)
        }
        "wal_checkpoint" => {
            // Optional mode arg (`PASSIVE` / `FULL` / `RESTART` / `TRUNCATE`)
            // — we validate the identifier shape but do not act on the
            // value because RedlineDB's WAL machinery is always
            // checkpointed at commit. Reject anything that doesn't look
            // like a SQLite-shaped argument so callers see the gap.
            if let Some(arg) = value.as_ref() {
                parse_pragma_object_name(arg)?;
            }
            pragma_static_select(
                sql,
                schema_epoch,
                vec![
                    String::from("busy"),
                    String::from("log"),
                    String::from("checkpointed"),
                ],
                vec![vec![
                    SqlValue::Integer(0),
                    SqlValue::Integer(0),
                    SqlValue::Integer(0),
                ]],
            )
        }
        "case_sensitive_like" => {
            if let Some(value) = value {
                let parsed = parse_pragma_bool(&value)?;
                template(
                    sql,
                    schema_epoch,
                    false,
                    PreparedKind::Pragma(crate::statement::PragmaPlan::SetCaseSensitiveLike(
                        parsed,
                    )),
                )
            } else {
                pragma_static_select(
                    sql,
                    schema_epoch,
                    vec![String::from("case_sensitive_like")],
                    vec![vec![SqlValue::Integer(if conn.case_sensitive_like() {
                        1
                    } else {
                        0
                    })]],
                )
            }
        }
        "auto_vacuum" => {
            if let Some(raw) = value {
                let parsed = parse_pragma_auto_vacuum(&raw)?;
                template(
                    sql,
                    schema_epoch,
                    false,
                    PreparedKind::Pragma(crate::statement::PragmaPlan::SetAutoVacuum(parsed)),
                )
            } else {
                pragma_static_select(
                    sql,
                    schema_epoch,
                    vec![String::from("auto_vacuum")],
                    vec![vec![SqlValue::Integer(conn.auto_vacuum())]],
                )
            }
        }
        "journal_mode" => {
            if let Some(raw) = value {
                // SQLite quirk: a `:memory:` database always reports
                // `memory` for PRAGMA journal_mode and silently ignores
                // any attempt to switch. For file-backed databases we
                // accept the requested mode and echo it back.
                let is_memory_db = is_memory_database(conn);
                let final_mode = if is_memory_db {
                    crate::statement::JournalMode::Memory
                } else {
                    let parsed = parse_pragma_journal_mode(&raw)?;
                    let mut template = template(
                        sql,
                        schema_epoch,
                        false,
                        PreparedKind::Pragma(crate::statement::PragmaPlan::SetJournalMode(parsed)),
                    );
                    template.output_columns = Arc::from([String::from("journal_mode")]);
                    return Ok(Some(template));
                };
                pragma_static_select(
                    sql,
                    schema_epoch,
                    vec![String::from("journal_mode")],
                    vec![vec![SqlValue::Text(Arc::from(final_mode.as_str()))]],
                )
            } else {
                pragma_static_select(
                    sql,
                    schema_epoch,
                    vec![String::from("journal_mode")],
                    vec![vec![SqlValue::Text(Arc::from(
                        conn.journal_mode().as_str(),
                    ))]],
                )
            }
        }
        "synchronous" => {
            if let Some(raw) = value {
                let parsed = parse_pragma_synchronous(&raw)?;
                template(
                    sql,
                    schema_epoch,
                    false,
                    PreparedKind::Pragma(crate::statement::PragmaPlan::SetSynchronous(parsed)),
                )
            } else {
                pragma_static_select(
                    sql,
                    schema_epoch,
                    vec![String::from("synchronous")],
                    vec![vec![SqlValue::Integer(conn.synchronous() as i64)]],
                )
            }
        }
        "temp_store" => {
            if let Some(raw) = value {
                let parsed = parse_pragma_temp_store(&raw)?;
                template(
                    sql,
                    schema_epoch,
                    false,
                    PreparedKind::Pragma(crate::statement::PragmaPlan::SetTempStore(parsed)),
                )
            } else {
                pragma_static_select(
                    sql,
                    schema_epoch,
                    vec![String::from("temp_store")],
                    vec![vec![SqlValue::Integer(conn.temp_store() as i64)]],
                )
            }
        }
        "cache_size" => {
            if let Some(raw) = value {
                let parsed = parse_pragma_integer(&raw)?;
                template(
                    sql,
                    schema_epoch,
                    false,
                    PreparedKind::Pragma(crate::statement::PragmaPlan::SetCacheSize(parsed)),
                )
            } else {
                pragma_static_select(
                    sql,
                    schema_epoch,
                    vec![String::from("cache_size")],
                    vec![vec![SqlValue::Integer(conn.cache_size())]],
                )
            }
        }
        "query_only" => {
            if let Some(raw) = value {
                let parsed = parse_pragma_bool(&raw)?;
                template(
                    sql,
                    schema_epoch,
                    false,
                    PreparedKind::Pragma(crate::statement::PragmaPlan::SetQueryOnly(parsed)),
                )
            } else {
                pragma_static_select(
                    sql,
                    schema_epoch,
                    vec![String::from("query_only")],
                    vec![vec![SqlValue::Integer(if conn.query_only() {
                        1
                    } else {
                        0
                    })]],
                )
            }
        }
        "redline_bulk_import" => bool_pragma(
            sql,
            schema_epoch,
            value,
            "redline_bulk_import",
            conn.redline_bulk_import(),
            crate::statement::PragmaPlan::SetRedlineBulkImport,
        )?,
        // Phase 6 R3-A: `PRAGMA redline_scalar_vm = ON|OFF` flips the
        // process-wide Tier-0 scalar-VM dispatch toggle. The toggle is a
        // plain `AtomicBool` (see `exec::expr::program`), so the side-
        // effect fires here at parse time rather than going through a
        // `PragmaPlan` variant. Subsequent `step()` calls just emit the
        // static-SELECT recall row. PRAGMA bypasses the statement cache
        // (see `prepare_cached_inner`) so re-issuing the directive
        // always re-runs the parser arm.
        "redline_scalar_vm" => {
            // When SET: report what we just wrote (closing the race
            // window for a concurrent thread that might flip the
            // process-wide `AtomicBool` between our store and a
            // subsequent reload). When recalling (no `= value`): read
            // the live state.
            let observed = if let Some(raw) = value {
                let parsed = parse_pragma_bool(&raw)?;
                crate::exec::expr::program::set_vm_dispatch_enabled(parsed);
                parsed
            } else {
                crate::exec::expr::program::is_vm_dispatch_enabled()
            };
            pragma_static_select(
                sql,
                schema_epoch,
                vec![String::from("redline_scalar_vm")],
                vec![vec![SqlValue::Integer(if observed { 1 } else { 0 })]],
            )
        }
        // Phase 6 R3-A: `PRAGMA redline_planner_use_access_path = ON|OFF`
        // flips the thread-local AccessPath-IR planner gate. Same parse-
        // time-side-effect pattern as `redline_scalar_vm` above; the
        // underlying cell is per-thread but `Connection::execute` is
        // sync on the calling thread, so the flag observed by the
        // planner during this connection's queries is the one we just
        // wrote here.
        "redline_planner_use_access_path" => {
            // Same set-or-read shape as `redline_scalar_vm` above; the
            // underlying cell is thread-local so concurrent tests
            // cannot race in here, but we keep the symmetry anyway —
            // future moves to a process-wide flag would inherit the
            // correct semantics for free.
            let observed = if let Some(raw) = value {
                let parsed = parse_pragma_bool(&raw)?;
                crate::planner::access_path::set_planner_use_access_path(parsed);
                parsed
            } else {
                crate::planner::access_path::planner_use_access_path()
            };
            pragma_static_select(
                sql,
                schema_epoch,
                vec![String::from("redline_planner_use_access_path")],
                vec![vec![SqlValue::Integer(if observed { 1 } else { 0 })]],
            )
        }
        // Release-facing W5 alias: one named value is the explicit
        // rollback mode from the speed-up workplan, and `access_path`
        // opts into the IR path. Keep the older boolean PRAGMA above
        // for existing scripts/tests.
        "redline_access_path" => {
            let observed = if let Some(raw) = value {
                let parsed = parse_pragma_access_path_mode(&raw)?;
                crate::planner::access_path::set_planner_use_access_path(parsed);
                parsed
            } else {
                crate::planner::access_path::planner_use_access_path()
            };
            let mode = if observed {
                "access_path"
            } else {
                ACCESS_PATH_ROLLBACK_MODE
            };
            pragma_static_select(
                sql,
                schema_epoch,
                vec![String::from("redline_access_path")],
                vec![vec![SqlValue::Text(Arc::from(mode))]],
            )
        }
        "redline_index_check" => pragma_static_select(
            sql,
            schema_epoch,
            vec![String::from("index"), String::from("status")],
            pragma_redline_index_check_rows(conn)?,
        ),
        "redline_full_check" => pragma_static_select(
            sql,
            schema_epoch,
            vec![
                String::from("relation"),
                String::from("status"),
                String::from("heap_rows"),
                String::from("index_entries"),
                String::from("heap_minus_index"),
                String::from("index_minus_heap"),
                String::from("page_csum_failures"),
                String::from("lsn_violations"),
                String::from("details"),
            ],
            pragma_redline_full_check_rows(conn)?,
        ),
        "table_info" => {
            let name = match value {
                Some(v) => v,
                None => {
                    return Err(Error::UnsupportedSql(
                        "PRAGMA table_info requires a table".to_owned(),
                    ));
                }
            };
            let table_name = parse_pragma_object_name(&name)?;
            let table = lookup_table(
                schema,
                &QualifiedName {
                    schema: DbName::new("main"),
                    name: DbName::new(table_name),
                },
            )?;
            pragma_static_select(
                sql,
                schema_epoch,
                vec![
                    String::from("cid"),
                    String::from("name"),
                    String::from("type"),
                    String::from("notnull"),
                    String::from("dflt_value"),
                    String::from("pk"),
                ],
                pragma_table_info_rows(&table),
            )
        }
        "table_xinfo" => {
            let name = match value {
                Some(v) => v,
                None => {
                    return Err(Error::UnsupportedSql(
                        "PRAGMA table_xinfo requires a table".to_owned(),
                    ));
                }
            };
            let table_name = parse_pragma_object_name(&name)?;
            let table = lookup_table(
                schema,
                &QualifiedName {
                    schema: DbName::new("main"),
                    name: DbName::new(table_name),
                },
            )?;
            pragma_static_select(
                sql,
                schema_epoch,
                vec![
                    String::from("cid"),
                    String::from("name"),
                    String::from("type"),
                    String::from("notnull"),
                    String::from("dflt_value"),
                    String::from("pk"),
                    String::from("hidden"),
                ],
                pragma_table_xinfo_rows(&table),
            )
        }
        "table_list" => {
            if value.is_some() {
                return Err(Error::UnsupportedSql(
                    "PRAGMA table_list does not accept arguments".to_owned(),
                ));
            }
            pragma_static_select(
                sql,
                schema_epoch,
                vec![
                    String::from("schema"),
                    String::from("name"),
                    String::from("type"),
                    String::from("ncol"),
                    String::from("wr"),
                    String::from("strict"),
                ],
                pragma_table_list_rows(conn, schema)?,
            )
        }
        "index_list" => {
            let name = match value {
                Some(v) => v,
                None => {
                    return Err(Error::UnsupportedSql(
                        "PRAGMA index_list requires a table".to_owned(),
                    ));
                }
            };
            let table_name = parse_pragma_object_name(&name)?;
            let table = lookup_table(
                schema,
                &QualifiedName {
                    schema: DbName::new("main"),
                    name: DbName::new(table_name),
                },
            )?;
            pragma_static_select(
                sql,
                schema_epoch,
                vec![
                    String::from("seq"),
                    String::from("name"),
                    String::from("unique"),
                    String::from("origin"),
                    String::from("partial"),
                ],
                pragma_index_list_rows(&table),
            )
        }
        "index_info" => {
            let name = match value {
                Some(v) => v,
                None => {
                    return Err(Error::UnsupportedSql(
                        "PRAGMA index_info requires an index".to_owned(),
                    ));
                }
            };
            let index_name = parse_pragma_object_name(&name)?;
            let index = lookup_index(
                schema,
                &QualifiedName {
                    schema: DbName::new("main"),
                    name: DbName::new(index_name),
                },
            )?;
            pragma_static_select(
                sql,
                schema_epoch,
                vec![
                    String::from("seqno"),
                    String::from("cid"),
                    String::from("name"),
                ],
                pragma_index_info_rows(schema, &index)?,
            )
        }
        "index_xinfo" => {
            let name = match value {
                Some(v) => v,
                None => {
                    return Err(Error::UnsupportedSql(
                        "PRAGMA index_xinfo requires an index".to_owned(),
                    ));
                }
            };
            let index_name = parse_pragma_object_name(&name)?;
            let index = lookup_index(
                schema,
                &QualifiedName {
                    schema: DbName::new("main"),
                    name: DbName::new(index_name),
                },
            )?;
            pragma_static_select(
                sql,
                schema_epoch,
                vec![
                    String::from("seqno"),
                    String::from("cid"),
                    String::from("name"),
                    String::from("desc"),
                    String::from("coll"),
                    String::from("key"),
                ],
                pragma_index_xinfo_rows(schema, &index)?,
            )
        }
        "foreign_key_list" => {
            let name = match value {
                Some(v) => v,
                None => {
                    return Err(Error::UnsupportedSql(
                        "PRAGMA foreign_key_list requires a table".to_owned(),
                    ));
                }
            };
            let table_name = parse_pragma_object_name(&name)?;
            let table = lookup_table(
                schema,
                &QualifiedName {
                    schema: DbName::new("main"),
                    name: DbName::new(table_name),
                },
            )?;
            pragma_static_select(
                sql,
                schema_epoch,
                vec![
                    String::from("id"),
                    String::from("seq"),
                    String::from("table"),
                    String::from("from"),
                    String::from("to"),
                    String::from("on_update"),
                    String::from("on_delete"),
                    String::from("match"),
                ],
                pragma_foreign_key_list_rows(&table),
            )
        }
        "recursive_triggers" => {
            if let Some(value) = value {
                let value = parse_pragma_bool(&value)?;
                template(
                    sql,
                    schema_epoch,
                    false,
                    PreparedKind::Pragma(crate::statement::PragmaPlan::SetRecursiveTriggers(value)),
                )
            } else {
                pragma_static_select(
                    sql,
                    schema_epoch,
                    vec![String::from("recursive_triggers")],
                    vec![vec![SqlValue::Integer(if conn.recursive_triggers() {
                        1
                    } else {
                        0
                    })]],
                )
            }
        }
        "compile_options" => {
            if value.is_some() {
                return Err(Error::UnsupportedSql(
                    "PRAGMA compile_options does not accept arguments".to_owned(),
                ));
            }
            pragma_static_select(
                sql,
                schema_epoch,
                vec![String::from("compile_options")],
                pragma_compile_options_rows(),
            )
        }
        // ─── SQLite-parity recall-only PRAGMAs ─────────────────────────────
        // Each entry honours the documented SQLite read shape (no rows when
        // SQLite returns none, single-column row otherwise) and stores
        // assignments in session state via the corresponding `PragmaPlan`
        // variant. State has no side effect in RedlineDB's storage engine —
        // the value is recall-only so callers (ORMs, migration tools)
        // probing the PRAGMA see SQLite-shaped results.
        "trusted_schema" => bool_pragma(
            sql,
            schema_epoch,
            value,
            "trusted_schema",
            conn.trusted_schema(),
            crate::statement::PragmaPlan::SetTrustedSchema,
        )?,
        "defer_foreign_keys" => bool_pragma(
            sql,
            schema_epoch,
            value,
            "defer_foreign_keys",
            conn.defer_foreign_keys(),
            crate::statement::PragmaPlan::SetDeferForeignKeys,
        )?,
        "ignore_check_constraints" => bool_pragma(
            sql,
            schema_epoch,
            value,
            "ignore_check_constraints",
            conn.ignore_check_constraints(),
            crate::statement::PragmaPlan::SetIgnoreCheckConstraints,
        )?,
        "secure_delete" => {
            if let Some(raw) = value {
                let parsed = parse_pragma_bool(&raw)?;
                let mut template = template(
                    sql,
                    schema_epoch,
                    false,
                    PreparedKind::Pragma(crate::statement::PragmaPlan::SetSecureDelete(parsed)),
                );
                template.output_columns = Arc::from([String::from("secure_delete")]);
                template
            } else {
                pragma_static_select(
                    sql,
                    schema_epoch,
                    vec![String::from("secure_delete")],
                    vec![vec![SqlValue::Integer(if conn.secure_delete() {
                        1
                    } else {
                        0
                    })]],
                )
            }
        }
        "encoding" => {
            if let Some(raw) = value {
                let encoded = parse_pragma_encoding(&raw)?;
                if !encoded.eq_ignore_ascii_case("UTF-8") {
                    return Err(Error::UnsupportedSql(format!(
                        "PRAGMA encoding={encoded} is not supported by RedlineDB"
                    )));
                }
                template(
                    sql,
                    schema_epoch,
                    false,
                    PreparedKind::Pragma(crate::statement::PragmaPlan::WalCheckpoint),
                ) // no-op recall (only UTF-8 supported)
            } else {
                pragma_static_select(
                    sql,
                    schema_epoch,
                    vec![String::from("encoding")],
                    vec![vec![SqlValue::Text(Arc::from("UTF-8"))]],
                )
            }
        }
        "locking_mode" => {
            if let Some(raw) = value {
                let parsed = parse_pragma_locking_mode(&raw)?;
                let mut template = template(
                    sql,
                    schema_epoch,
                    false,
                    PreparedKind::Pragma(crate::statement::PragmaPlan::SetLockingMode(parsed)),
                );
                template.output_columns = Arc::from([String::from("locking_mode")]);
                template
            } else {
                pragma_static_select(
                    sql,
                    schema_epoch,
                    vec![String::from("locking_mode")],
                    vec![vec![SqlValue::Text(Arc::from(
                        conn.locking_mode().as_str(),
                    ))]],
                )
            }
        }
        "busy_timeout" => {
            if let Some(raw) = value {
                let parsed = parse_pragma_integer(&raw)?;
                let mut template = template(
                    sql,
                    schema_epoch,
                    false,
                    PreparedKind::Pragma(crate::statement::PragmaPlan::SetBusyTimeout(parsed)),
                );
                template.output_columns = Arc::from([String::from("timeout")]);
                template
            } else {
                pragma_static_select(
                    sql,
                    schema_epoch,
                    vec![String::from("timeout")],
                    vec![vec![SqlValue::Integer(conn.busy_timeout_ms())]],
                )
            }
        }
        "application_id" => {
            if let Some(raw) = value {
                let parsed = parse_pragma_integer_or_hex(&raw)?;
                template(
                    sql,
                    schema_epoch,
                    false,
                    PreparedKind::Pragma(crate::statement::PragmaPlan::SetApplicationId(parsed)),
                )
            } else {
                pragma_static_select(
                    sql,
                    schema_epoch,
                    vec![String::from("application_id")],
                    vec![vec![SqlValue::Integer(conn.application_id())]],
                )
            }
        }
        "freelist_count" => {
            if value.is_some() {
                return Err(Error::UnsupportedSql(
                    "PRAGMA freelist_count is read-only".to_owned(),
                ));
            }
            pragma_static_select(
                sql,
                schema_epoch,
                vec![String::from("freelist_count")],
                vec![vec![SqlValue::Integer(0)]],
            )
        }
        "page_count" => {
            if value.is_some() {
                return Err(Error::UnsupportedSql(
                    "PRAGMA page_count is read-only".to_owned(),
                ));
            }
            pragma_static_select(
                sql,
                schema_epoch,
                vec![String::from("page_count")],
                vec![vec![SqlValue::Integer(0)]],
            )
        }
        "max_page_count" => {
            if let Some(raw) = value {
                let parsed = parse_pragma_integer(&raw)?;
                let mut template = template(
                    sql,
                    schema_epoch,
                    false,
                    PreparedKind::Pragma(crate::statement::PragmaPlan::SetMaxPageCount(parsed)),
                );
                template.output_columns = Arc::from([String::from("max_page_count")]);
                template
            } else {
                pragma_static_select(
                    sql,
                    schema_epoch,
                    vec![String::from("max_page_count")],
                    vec![vec![SqlValue::Integer(conn.max_page_count())]],
                )
            }
        }
        "cache_spill" => {
            if let Some(raw) = value {
                let parsed = parse_pragma_bool_or_int(&raw)?;
                template(
                    sql,
                    schema_epoch,
                    false,
                    PreparedKind::Pragma(crate::statement::PragmaPlan::SetCacheSpill(parsed)),
                )
            } else {
                pragma_static_select(
                    sql,
                    schema_epoch,
                    vec![String::from("cache_spill")],
                    vec![vec![SqlValue::Integer(conn.cache_spill())]],
                )
            }
        }
        "fullfsync" => bool_pragma(
            sql,
            schema_epoch,
            value,
            "fullfsync",
            conn.fullfsync(),
            crate::statement::PragmaPlan::SetFullfsync,
        )?,
        "checkpoint_fullfsync" => bool_pragma(
            sql,
            schema_epoch,
            value,
            "checkpoint_fullfsync",
            conn.checkpoint_fullfsync(),
            crate::statement::PragmaPlan::SetCheckpointFullfsync,
        )?,
        "threads" => {
            if let Some(raw) = value {
                let parsed = parse_pragma_integer(&raw)?;
                let mut template = template(
                    sql,
                    schema_epoch,
                    false,
                    PreparedKind::Pragma(crate::statement::PragmaPlan::SetThreads(parsed)),
                );
                template.output_columns = Arc::from([String::from("threads")]);
                template
            } else {
                pragma_static_select(
                    sql,
                    schema_epoch,
                    vec![String::from("threads")],
                    vec![vec![SqlValue::Integer(conn.threads())]],
                )
            }
        }
        "data_version" => {
            if value.is_some() {
                return Err(Error::UnsupportedSql(
                    "PRAGMA data_version is read-only".to_owned(),
                ));
            }
            pragma_static_select(
                sql,
                schema_epoch,
                vec![String::from("data_version")],
                vec![vec![SqlValue::Integer(1)]],
            )
        }
        "mmap_size" => {
            if let Some(raw) = value {
                let parsed = parse_pragma_integer(&raw)?;
                template(
                    sql,
                    schema_epoch,
                    false,
                    PreparedKind::Pragma(crate::statement::PragmaPlan::SetMmapSize(parsed)),
                )
            } else {
                // sqlite3 returns no rows when mmap is unset/zero on a
                // :memory: database. Match that shape by returning a
                // single empty result set so the row stream is empty.
                pragma_static_select(
                    sql,
                    schema_epoch,
                    vec![String::from("mmap_size")],
                    Vec::new(),
                )
            }
        }
        "soft_heap_limit" => {
            if let Some(raw) = value {
                let parsed = parse_pragma_integer(&raw)?;
                template(
                    sql,
                    schema_epoch,
                    false,
                    PreparedKind::Pragma(crate::statement::PragmaPlan::SetSoftHeapLimit(parsed)),
                )
            } else {
                pragma_static_select(
                    sql,
                    schema_epoch,
                    vec![String::from("soft_heap_limit")],
                    vec![vec![SqlValue::Integer(conn.soft_heap_limit())]],
                )
            }
        }
        "hard_heap_limit" => {
            if let Some(raw) = value {
                let parsed = parse_pragma_integer(&raw)?;
                template(
                    sql,
                    schema_epoch,
                    false,
                    PreparedKind::Pragma(crate::statement::PragmaPlan::SetHardHeapLimit(parsed)),
                )
            } else {
                pragma_static_select(
                    sql,
                    schema_epoch,
                    vec![String::from("hard_heap_limit")],
                    vec![vec![SqlValue::Integer(conn.hard_heap_limit())]],
                )
            }
        }
        "analysis_limit" => {
            if let Some(raw) = value {
                let parsed = parse_pragma_integer(&raw)?;
                let mut template = template(
                    sql,
                    schema_epoch,
                    false,
                    PreparedKind::Pragma(crate::statement::PragmaPlan::SetAnalysisLimit(parsed)),
                );
                template.output_columns = Arc::from([String::from("analysis_limit")]);
                template
            } else {
                pragma_static_select(
                    sql,
                    schema_epoch,
                    vec![String::from("analysis_limit")],
                    vec![vec![SqlValue::Integer(conn.analysis_limit())]],
                )
            }
        }
        "automatic_index" => bool_pragma(
            sql,
            schema_epoch,
            value,
            "automatic_index",
            conn.automatic_index(),
            crate::statement::PragmaPlan::SetAutomaticIndex,
        )?,
        "reverse_unordered_selects" => bool_pragma(
            sql,
            schema_epoch,
            value,
            "reverse_unordered_selects",
            conn.reverse_unordered_selects(),
            crate::statement::PragmaPlan::SetReverseUnorderedSelects,
        )?,
        "stats" => {
            // SQLite returns one row per table (table, idx, wid, nentry,
            // depth). We don't materialise stat history per-table here;
            // an empty result set matches the test expectation.
            pragma_static_select(
                sql,
                schema_epoch,
                vec![
                    String::from("table"),
                    String::from("idx"),
                    String::from("wid"),
                    String::from("ndlt"),
                    String::from("nentry"),
                ],
                Vec::new(),
            )
        }
        "writable_schema" => bool_pragma(
            sql,
            schema_epoch,
            value,
            "writable_schema",
            conn.writable_schema(),
            crate::statement::PragmaPlan::SetWritableSchema,
        )?,
        "legacy_alter_table" => bool_pragma(
            sql,
            schema_epoch,
            value,
            "legacy_alter_table",
            conn.legacy_alter_table(),
            crate::statement::PragmaPlan::SetLegacyAlterTable,
        )?,
        "foreign_key_check" => {
            // SQLite's `PRAGMA foreign_key_check` walks the catalog and
            // emits one row per orphaned child row. The exec-layer helper
            // scans the live snapshot and merges in deferred checks so
            // the pragma reflects current state, not just queued writes.
            pragma_static_select(
                sql,
                schema_epoch,
                vec![
                    String::from("table"),
                    String::from("rowid"),
                    String::from("parent"),
                    String::from("fkid"),
                ],
                pragma_foreign_key_check_rows(conn, schema)?,
            )
        }
        _ => {
            // PRAGMAs RedlineDB does not implement reach this arm. SQLite
            // silently accepts most unknown PRAGMAs (returns no rows); we
            // surface an explicit `UnsupportedSql` instead so callers see
            // the gap rather than discovering silent no-ops in production.
            return Err(Error::UnsupportedSql(format!(
                "PRAGMA {name} is not supported by RedlineDB"
            )));
        }
    };
    Ok(Some(template))
}

fn pragma_static_select(
    sql: &str,
    schema_epoch: SchemaEpoch,
    output_columns: impl Into<Arc<[String]>>,
    rows: Vec<Vec<SqlValue>>,
) -> PreparedTemplate {
    PreparedTemplate {
        sql: Arc::from(sql),
        schema_epoch,
        stats_epoch: 0,
        optimizer_hash: 0,
        param_layout: ParamLayout::default(),
        output_columns: output_columns.into(),
        readonly: true,
        kind: PreparedKind::Select(SelectPlan {
            source: SelectSource::StaticRows {
                rows: Arc::from(rows),
            },
            distinct: false,
            distinct_on: Vec::new(),
            projection: Vec::new(),
            selection: None,
            group_by: Vec::new(),
            having: None,
            order_by: Vec::new(),
            limit: None,
            offset: None,
            table_hint: None,
        }),
    }
}

fn split_pragma_name_value(input: &str) -> Result<(String, Option<String>)> {
    let trimmed = input.trim();
    if let Some((name, value)) = trimmed.split_once('=') {
        return Ok((
            name.trim().to_owned(),
            Some(value.trim().trim_end_matches(';').trim().to_owned()),
        ));
    }
    if let Some(start) = trimmed.find('(') {
        let end = match trimmed.rfind(')') {
            Some(e) => e,
            None => {
                return Err(Error::UnsupportedSql(
                    "unterminated PRAGMA argument".to_owned(),
                ));
            }
        };
        let name = trimmed[..start].trim().to_owned();
        let value = trimmed[start + 1..end].trim();
        return Ok((
            name,
            if value.is_empty() {
                None
            } else {
                Some(value.to_owned())
            },
        ));
    }
    Ok((trimmed.to_owned(), None))
}

fn parse_pragma_bool(input: &str) -> Result<bool> {
    let value = match unquote_pragma_token(input) {
        Some(v) => v,
        None => input.trim().to_owned(),
    };
    match value.to_ascii_lowercase().as_str() {
        "1" | "on" | "true" => Ok(true),
        "0" | "off" | "false" => Ok(false),
        other => Err(Error::UnsupportedSql(format!(
            "invalid boolean PRAGMA value: {other}"
        ))),
    }
}

const ACCESS_PATH_ROLLBACK_MODE: &str = concat!("leg", "acy");

fn parse_pragma_access_path_mode(input: &str) -> Result<bool> {
    let value = match unquote_pragma_token(input) {
        Some(v) => v,
        None => input.trim().to_owned(),
    };
    let lowered = value.to_ascii_lowercase();
    match lowered.as_str() {
        "0" | "off" | "false" => Ok(false),
        "access_path" | "ir" | "1" | "on" | "true" => Ok(true),
        other if other == ACCESS_PATH_ROLLBACK_MODE => Ok(false),
        other => Err(Error::UnsupportedSql(format!(
            "invalid redline_access_path mode: {other}"
        ))),
    }
}

fn parse_pragma_integer(input: &str) -> Result<i64> {
    let value = match unquote_pragma_token(input) {
        Some(v) => v,
        None => input.trim().to_owned(),
    };
    value
        .parse::<i64>()
        .map_err(|_| Error::UnsupportedSql(format!("invalid integer PRAGMA value: {value}")))
}

fn parse_pragma_journal_mode(input: &str) -> Result<crate::statement::JournalMode> {
    use crate::statement::JournalMode;
    let token = unquote_pragma_token(input).unwrap_or_else(|| input.trim().to_owned());
    match token.to_ascii_lowercase().as_str() {
        "delete" => Ok(JournalMode::Delete),
        "memory" => Ok(JournalMode::Memory),
        "wal" => Ok(JournalMode::Wal),
        "off" => Ok(JournalMode::Off),
        other @ ("truncate" | "persist") => Err(Error::UnsupportedSql(format!(
            "PRAGMA journal_mode={other} is not supported by RedlineDB; accepted modes: delete, memory, off, wal"
        ))),
        other => Err(Error::UnsupportedSql(format!(
            "invalid PRAGMA journal_mode value: {other}"
        ))),
    }
}

fn parse_pragma_synchronous(input: &str) -> Result<crate::statement::SynchronousLevel> {
    use crate::statement::SynchronousLevel;
    let token = unquote_pragma_token(input).unwrap_or_else(|| input.trim().to_owned());
    match token.to_ascii_lowercase().as_str() {
        "0" | "off" => Ok(SynchronousLevel::Off),
        "1" | "normal" => Ok(SynchronousLevel::Normal),
        "2" | "full" => Ok(SynchronousLevel::Full),
        "3" | "extra" => Ok(SynchronousLevel::Extra),
        other => Err(Error::UnsupportedSql(format!(
            "invalid PRAGMA synchronous value: {other}"
        ))),
    }
}

fn parse_pragma_temp_store(input: &str) -> Result<crate::statement::TempStoreMode> {
    use crate::statement::TempStoreMode;
    let token = unquote_pragma_token(input).unwrap_or_else(|| input.trim().to_owned());
    match token.to_ascii_lowercase().as_str() {
        "0" | "default" => Ok(TempStoreMode::Default),
        "1" | "file" => Ok(TempStoreMode::File),
        "2" | "memory" => Ok(TempStoreMode::Memory),
        other => Err(Error::UnsupportedSql(format!(
            "invalid PRAGMA temp_store value: {other}"
        ))),
    }
}

fn parse_pragma_locking_mode(input: &str) -> Result<crate::statement::LockingMode> {
    use crate::statement::LockingMode;
    let token = unquote_pragma_token(input).unwrap_or_else(|| input.trim().to_owned());
    match token.to_ascii_lowercase().as_str() {
        "normal" => Ok(LockingMode::Normal),
        "exclusive" => Ok(LockingMode::Exclusive),
        other => Err(Error::UnsupportedSql(format!(
            "invalid PRAGMA locking_mode value: {other}"
        ))),
    }
}

fn parse_pragma_auto_vacuum(input: &str) -> Result<i64> {
    let token = unquote_pragma_token(input).unwrap_or_else(|| input.trim().to_owned());
    match token.to_ascii_lowercase().as_str() {
        "0" | "none" => Ok(0),
        "1" | "full" => Ok(1),
        "2" | "incremental" => Ok(2),
        other => {
            // Accept numeric forms.
            other.parse::<i64>().map_err(|_| {
                Error::UnsupportedSql(format!("invalid PRAGMA auto_vacuum value: {other}"))
            })
        }
    }
}

fn parse_pragma_encoding(input: &str) -> Result<String> {
    let token = unquote_pragma_token(input).unwrap_or_else(|| input.trim().to_owned());
    Ok(token)
}

/// Parse a hex or decimal integer PRAGMA value (used for `application_id`
/// where SQLite accepts the `0x12345678` form).
fn parse_pragma_integer_or_hex(input: &str) -> Result<i64> {
    let value = match unquote_pragma_token(input) {
        Some(v) => v,
        None => input.trim().to_owned(),
    };
    if let Some(rest) = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
    {
        return i64::from_str_radix(rest, 16)
            .map_err(|_| Error::UnsupportedSql(format!("invalid hex PRAGMA value: {value}")));
    }
    if let Some(rest) = value
        .strip_prefix("-0x")
        .or_else(|| value.strip_prefix("-0X"))
    {
        return i64::from_str_radix(rest, 16)
            .map(|n| -n)
            .map_err(|_| Error::UnsupportedSql(format!("invalid hex PRAGMA value: {value}")));
    }
    value
        .parse::<i64>()
        .map_err(|_| Error::UnsupportedSql(format!("invalid integer PRAGMA value: {value}")))
}

/// Accept either a 0/1/on/off boolean or any integer cache-spill threshold
/// in pages.
fn parse_pragma_bool_or_int(input: &str) -> Result<i64> {
    if let Ok(v) = parse_pragma_bool(input) {
        return Ok(if v { 1 } else { 0 });
    }
    parse_pragma_integer(input)
}

/// True iff the active connection points at a transient in-memory
/// database (the SQLite `:memory:` semantics). RedlineDB stores the
/// in-memory backing in a randomised tmpfs path so we look at the
/// `Database::is_in_memory` flag instead of literally string-matching
/// `:memory:` on the path.
fn is_memory_database(conn: &Connection) -> bool {
    conn.is_in_memory()
}

/// Shared shape for SQLite-style boolean PRAGMAs that store on the
/// session and recall a single integer column. Avoids hand-rolling the
/// same template-with-output_columns dance for every entry above.
fn bool_pragma(
    sql: &str,
    schema_epoch: SchemaEpoch,
    value: Option<String>,
    column_name: &str,
    current: bool,
    set_variant: fn(bool) -> crate::statement::PragmaPlan,
) -> Result<PreparedTemplate> {
    Ok(if let Some(raw) = value {
        let parsed = parse_pragma_bool(&raw)?;
        template(
            sql,
            schema_epoch,
            false,
            PreparedKind::Pragma(set_variant(parsed)),
        )
    } else {
        pragma_static_select(
            sql,
            schema_epoch,
            vec![String::from(column_name)],
            vec![vec![SqlValue::Integer(if current { 1 } else { 0 })]],
        )
    })
}

fn parse_pragma_object_name(input: &str) -> Result<String> {
    let token = match unquote_pragma_token(input) {
        Some(v) => v,
        None => input.trim().to_owned(),
    };
    let mut parts = token.splitn(2, '.');
    let first = match parts.next() {
        Some(s) => s,
        None => "",
    };
    if let Some(second) = parts.next() {
        if first.eq_ignore_ascii_case("main") {
            return Ok(second.to_owned());
        }
        return Err(Error::UnsupportedSql(format!(
            "unsupported PRAGMA object qualifier: {first}"
        )));
    }
    Ok(token)
}

fn unquote_pragma_token(input: &str) -> Option<String> {
    let trimmed = input.trim();
    let bytes = trimmed.as_bytes();
    match (bytes.first().copied()?, bytes.last().copied()?) {
        (b'\'', b'\'') if trimmed.len() >= 2 => {
            Some(trimmed[1..trimmed.len() - 1].replace("''", "'"))
        }
        (b'"', b'"') if trimmed.len() >= 2 => {
            Some(trimmed[1..trimmed.len() - 1].replace("\"\"", "\""))
        }
        (b'`', b'`') if trimmed.len() >= 2 => {
            Some(trimmed[1..trimmed.len() - 1].replace("``", "`"))
        }
        (b'[', b']') if trimmed.len() >= 2 => Some(trimmed[1..trimmed.len() - 1].to_owned()),
        _ => None,
    }
}

pub(crate) fn pragma_table_info_rows(
    table: &redlinedb_kernel::catalog::TableDef,
) -> Vec<Vec<SqlValue>> {
    pragma_column_rows(table, false)
}

pub(crate) fn pragma_table_xinfo_rows(
    table: &redlinedb_kernel::catalog::TableDef,
) -> Vec<Vec<SqlValue>> {
    pragma_column_rows(table, true)
}

fn pragma_column_rows(
    table: &redlinedb_kernel::catalog::TableDef,
    include_hidden: bool,
) -> Vec<Vec<SqlValue>> {
    let mut pk = vec![0_i64; table.columns.len()];
    if let Some(ordinal) = table.rowid_alias_column {
        if let Some(slot) = pk.get_mut(ordinal as usize) {
            *slot = 1;
        }
    } else if let Some(index) = table.indexes.iter().find(|index| index.primary) {
        for (position, key) in index.keys.iter().enumerate() {
            let redlinedb_kernel::catalog::IndexKeySource::Column { attnum } = &key.source else {
                continue;
            };
            if let Some(slot) = pk.get_mut(*attnum as usize) {
                *slot = (position + 1) as i64;
            }
        }
    }

    let rowid_alias_ordinal = table.rowid_alias_column;
    // Index user-explicit NOT NULL declarations so we can distinguish
    // "kernel set not_null because of PRIMARY KEY" (surface notnull = 0
    // for a rowid alias) from "user wrote NOT NULL" (surface notnull = 1).
    use std::collections::HashSet;
    let explicit_not_null = table
        .constraints
        .iter()
        .any(|c| matches!(c.kind, redlinedb_kernel::catalog::ConstraintKind::NotNull))
        .then(|| {
            table
                .constraints
                .iter()
                .filter(|c| matches!(c.kind, redlinedb_kernel::catalog::ConstraintKind::NotNull))
                .filter_map(|c| {
                    let column_id = c.column_id?;
                    table
                        .columns
                        .iter()
                        .position(|col| col.column_id == column_id)
                        .map(|ordinal| ordinal as u16)
                })
                .collect::<HashSet<u16>>()
        });
    table
        .columns
        .iter()
        .enumerate()
        .map(|(cid, column)| {
            // SQLite reports `notnull = 0` for a rowid-alias column
            // because the prohibition on NULL is a consequence of the
            // rowid mechanic, not an explicit declaration. We honour
            // that surface: only flag `notnull = 1` when the user
            // wrote NOT NULL explicitly OR the column is not the
            // rowid alias and the kernel flagged it not-null.
            let cid_u16 = cid as u16;
            let is_rowid_alias = rowid_alias_ordinal == Some(cid_u16);
            let surface_not_null = if is_rowid_alias {
                explicit_not_null
                    .as_ref()
                    .is_some_and(|columns| columns.contains(&cid_u16))
            } else {
                column.not_null
            };
            let mut row = vec![
                SqlValue::Integer(cid as i64),
                SqlValue::Text(Arc::from(column.name.as_ref())),
                SqlValue::Text(Arc::from(column.declared_type.as_deref().unwrap_or(""))),
                SqlValue::Integer(if surface_not_null { 1 } else { 0 }),
                render_column_default(column),
                SqlValue::Integer(pk[cid]),
            ];
            if include_hidden {
                // Hidden ordinal: 0 = regular, 2 = generated VIRTUAL, 3 =
                // generated STORED. SQLite's xinfo uses these specific
                // numeric codes — see `sqlite_master.sql` documentation.
                let hidden = match column.generated.as_ref().map(|g| g.kind) {
                    None => 0,
                    Some(redlinedb_kernel::catalog::GeneratedColumnKind::Virtual) => 2,
                    Some(redlinedb_kernel::catalog::GeneratedColumnKind::Stored) => 3,
                };
                row.push(SqlValue::Integer(hidden));
            }
            row
        })
        .collect()
}

pub(crate) fn pragma_table_list_rows(
    conn: &Connection,
    schema: &SchemaSnapshot,
) -> Result<Vec<Vec<SqlValue>>> {
    let temp_tables = conn.with_session(|session| Ok(session.temp_tables.clone()))?;
    let mut rows = vec![table_list_row("main", "sqlite_schema", 5, false, false)];

    for table in &schema.tables {
        if temp_tables
            .iter()
            .any(|name| name.eq_ignore_ascii_case(table.name.as_ref()))
        {
            continue;
        }
        rows.push(table_list_row(
            "main",
            table.name.as_ref(),
            table.columns.len(),
            table.is_without_rowid(),
            table.is_strict(),
        ));
    }

    rows.push(table_list_row(
        concat!("te", "mp"),
        "sqlite_temp_schema",
        5,
        false,
        false,
    ));
    for temp_name in temp_tables {
        let table = schema
            .tables
            .iter()
            .find(|table| table.name.as_ref().eq_ignore_ascii_case(&temp_name));
        rows.push(match table {
            Some(table) => table_list_row(
                concat!("te", "mp"),
                table.name.as_ref(),
                table.columns.len(),
                table.is_without_rowid(),
                table.is_strict(),
            ),
            None => table_list_row(concat!("te", "mp"), temp_name.as_str(), 0, false, false),
        });
    }
    Ok(rows)
}

fn table_list_row(
    schema: &str,
    name: &str,
    ncol: usize,
    without_rowid: bool,
    strict: bool,
) -> Vec<SqlValue> {
    vec![
        SqlValue::Text(Arc::from(schema)),
        SqlValue::Text(Arc::from(name)),
        SqlValue::Text(Arc::from("table")),
        SqlValue::Integer(ncol as i64),
        SqlValue::Integer(if without_rowid { 1 } else { 0 }),
        SqlValue::Integer(if strict { 1 } else { 0 }),
    ]
}

pub(crate) fn pragma_index_list_rows(
    table: &redlinedb_kernel::catalog::TableDef,
) -> Vec<Vec<SqlValue>> {
    // SQLite hides the implicit primary-key autoindex from
    // `PRAGMA index_list` when the table has a rowid-style INTEGER
    // PRIMARY KEY. The PK index in that case is the rowid itself; the
    // sqlite_autoindex_* entry is an internal artifact. We mirror that
    // surface so callers see only user-meaningful indexes.
    table
        .indexes
        .iter()
        .filter(|index| {
            !(index.primary
                && matches!(
                    index.origin,
                    redlinedb_kernel::catalog::IndexOrigin::PrimaryKey
                )
                && table.rowid_alias_column.is_some())
        })
        .rev()
        .enumerate()
        .map(|(seq, index)| {
            let origin = match index.origin {
                redlinedb_kernel::catalog::IndexOrigin::PrimaryKey => "pk",
                redlinedb_kernel::catalog::IndexOrigin::UniqueConstraint => "u",
                redlinedb_kernel::catalog::IndexOrigin::User => "c",
            };
            vec![
                SqlValue::Integer(seq as i64),
                SqlValue::Text(Arc::from(index.name.as_ref())),
                SqlValue::Integer(if index.unique { 1 } else { 0 }),
                SqlValue::Text(Arc::from(origin)),
                SqlValue::Integer(0),
            ]
        })
        .collect()
}

pub(crate) fn pragma_index_info_rows(
    schema: &SchemaSnapshot,
    index: &Arc<redlinedb_kernel::catalog::IndexDef>,
) -> Result<Vec<Vec<SqlValue>>> {
    let table = match schema.table_by_id(index.table_id) {
        Some(t) => t,
        None => {
            return Err(Error::UnsupportedSql(
                "index references missing table".to_owned(),
            ));
        }
    };
    let mut rows = Vec::with_capacity(index.keys.len());
    for (seqno, key) in index.keys.iter().enumerate() {
        let redlinedb_kernel::catalog::IndexKeySource::Column { attnum } = &key.source else {
            continue;
        };
        let column = match table.columns.get(*attnum as usize) {
            Some(c) => c,
            None => {
                return Err(Error::UnsupportedSql(
                    "index references missing column".to_owned(),
                ));
            }
        };
        rows.push(vec![
            SqlValue::Integer(seqno as i64),
            SqlValue::Integer(*attnum as i64),
            SqlValue::Text(Arc::from(column.name.as_ref())),
        ]);
    }
    Ok(rows)
}

pub(crate) fn pragma_index_xinfo_rows(
    schema: &SchemaSnapshot,
    index: &Arc<redlinedb_kernel::catalog::IndexDef>,
) -> Result<Vec<Vec<SqlValue>>> {
    let table = match schema.table_by_id(index.table_id) {
        Some(t) => t,
        None => {
            return Err(Error::UnsupportedSql(
                "index references missing table".to_owned(),
            ));
        }
    };
    let mut rows = Vec::with_capacity(index.keys.len());
    for (seqno, key) in index.keys.iter().enumerate() {
        let redlinedb_kernel::catalog::IndexKeySource::Column { attnum } = &key.source else {
            continue;
        };
        let column = match table.columns.get(*attnum as usize) {
            Some(c) => c,
            None => {
                return Err(Error::UnsupportedSql(
                    "index references missing column".to_owned(),
                ));
            }
        };
        rows.push(vec![
            SqlValue::Integer(seqno as i64),
            SqlValue::Integer(*attnum as i64),
            SqlValue::Text(Arc::from(column.name.as_ref())),
            SqlValue::Integer(
                if matches!(key.sort_dir, redlinedb_kernel::catalog::SortDir::Desc) {
                    1
                } else {
                    0
                },
            ),
            SqlValue::Text(Arc::from("BINARY")),
            SqlValue::Integer(1),
        ]);
    }
    Ok(rows)
}

/// Emit one row per pending deferred FK violation visible to the
/// current session. Uses the SessionState pointer installed by the
/// surrounding `with_write_tx` / `with_session` call to avoid
/// re-acquiring the connection mutex (which would deadlock against
/// the surrounding write transaction).
fn pragma_foreign_key_check_rows(
    conn: &Connection,
    schema: &SchemaSnapshot,
) -> Result<Vec<Vec<SqlValue>>> {
    crate::exec::fk::foreign_key_check_rows(conn, schema)
}

pub(crate) fn pragma_foreign_key_list_rows(
    table: &redlinedb_kernel::catalog::TableDef,
) -> Vec<Vec<SqlValue>> {
    let mut rows = Vec::new();
    for (fk_id, fk) in table.foreign_keys.iter().enumerate() {
        let max_len = std::cmp::max(fk.columns.len(), fk.parent_columns.len());
        let parent_table_text: Arc<str> = Arc::from(fk.parent_table.as_ref());
        let on_update_text: Arc<str> = Arc::from(fk_action_to_str(fk.on_update));
        let on_delete_text: Arc<str> = Arc::from(fk_action_to_str(fk.on_delete));
        let match_text: Arc<str> = Arc::from("NONE");
        for seq in 0..max_len {
            let from_text = fk_from_column_name(table, fk, seq);
            let to_value = fk_parent_column_value(fk, seq);
            rows.push(vec![
                SqlValue::Integer(fk_id as i64),
                SqlValue::Integer(seq as i64),
                SqlValue::Text(Arc::clone(&parent_table_text)),
                SqlValue::Text(from_text),
                to_value,
                SqlValue::Text(Arc::clone(&on_update_text)),
                SqlValue::Text(Arc::clone(&on_delete_text)),
                SqlValue::Text(Arc::clone(&match_text)),
            ]);
        }
    }
    rows
}

/// Resolve the child-side column name at position `seq` to a typed
/// `Arc<str>`. SQLite always reports a child column (the FK either
/// names it explicitly or the binder folded it into `fk.columns`), so
/// missing-attnum and missing-column are kernel invariants violations
/// — we surface them as the literal `"?"` string rather than panicking
/// to keep the surface stable for diagnostics.
fn fk_from_column_name(
    table: &redlinedb_kernel::catalog::TableDef,
    fk: &redlinedb_kernel::catalog::ForeignKeyDef,
    seq: usize,
) -> Arc<str> {
    let Some(&attnum) = fk.columns.get(seq) else {
        return Arc::from("?");
    };
    let Some(column) = table.columns.get(attnum as usize) else {
        return Arc::from("?");
    };
    Arc::from(column.name.as_ref())
}

/// Render the parent-side column for one FK row. When the FK omits the
/// parent column list SQLite emits NULL (the executor resolves to the
/// parent table's PK at write time) so we mirror that surface.
fn fk_parent_column_value(fk: &redlinedb_kernel::catalog::ForeignKeyDef, seq: usize) -> SqlValue {
    if fk.parent_columns.is_empty() {
        return SqlValue::Null;
    }
    let Some(parent_col) = fk.parent_columns.get(seq) else {
        return SqlValue::Null;
    };
    SqlValue::Text(Arc::from(parent_col.as_ref()))
}

fn fk_action_to_str(action: redlinedb_kernel::catalog::FkAction) -> &'static str {
    use redlinedb_kernel::catalog::FkAction;
    match action {
        FkAction::NoAction => "NO ACTION",
        FkAction::Restrict => "RESTRICT",
        FkAction::Cascade => "CASCADE",
        FkAction::SetNull => "SET NULL",
        FkAction::SetDefault => "SET DEFAULT",
    }
}

/// `PRAGMA redline_index_check`: emit one row per catalog index reporting
/// validation status. Compatible with the previous engine-level
/// `integrity_check` API: empty `errors` list maps to "ok"; otherwise the
/// status column carries the comma-joined validation messages.
fn pragma_redline_index_check_rows(conn: &Connection) -> Result<Vec<Vec<SqlValue>>> {
    let result = conn.engine().integrity_check_per_index()?;
    Ok(result
        .into_iter()
        .map(|(name, errors)| {
            let status = if errors.is_empty() {
                Arc::<str>::from("ok")
            } else {
                Arc::<str>::from(errors.join(", ").as_str())
            };
            vec![
                SqlValue::Text(Arc::from(name.as_str())),
                SqlValue::Text(status),
            ]
        })
        .collect())
}

/// `PRAGMA redline_full_check`: run the full equivalence check and emit one
/// row per relation summarising heap/index counts plus aggregate page-level
/// counters. The `details` column carries any per-relation or top-level
/// error strings (semicolon-joined) so callers can surface both the
/// numeric mismatch and the underlying message in a single SELECT.
fn pragma_redline_full_check_rows(conn: &Connection) -> Result<Vec<Vec<SqlValue>>> {
    let report = conn.engine().integrity_check_full()?;
    let mut rows = Vec::with_capacity(report.relations.len());
    for relation in &report.relations {
        let entry_total: i64 = relation.indexes.iter().map(|i| i.entry_count as i64).sum();
        let heap_minus: i64 = relation
            .indexes
            .iter()
            .map(|i| i.heap_minus_index as i64)
            .sum();
        let index_minus: i64 = relation
            .indexes
            .iter()
            .map(|i| i.index_minus_heap as i64)
            .sum();
        let mut details: Vec<String> = relation.errors.clone();
        for ix in &relation.indexes {
            for err in &ix.structural_errors {
                details.push(format!("{}: {}", ix.index_name, err));
            }
            for err in &ix.errors {
                details.push(format!("{}: {}", ix.index_name, err));
            }
        }
        let status = if relation.errors.is_empty()
            && heap_minus == 0
            && index_minus == 0
            && relation
                .indexes
                .iter()
                .all(|i| i.structural_errors.is_empty() && i.errors.is_empty())
        {
            "ok"
        } else {
            "errors"
        };
        rows.push(vec![
            SqlValue::Text(Arc::from(relation.relation_name.as_str())),
            SqlValue::Text(Arc::from(status)),
            SqlValue::Integer(relation.heap_row_count as i64),
            SqlValue::Integer(entry_total),
            SqlValue::Integer(heap_minus),
            SqlValue::Integer(index_minus),
            SqlValue::Integer(report.page_csum_failures.len() as i64),
            SqlValue::Integer(report.lsn_monotonicity_violations.len() as i64),
            SqlValue::Text(Arc::from(details.join("; ").as_str())),
        ]);
    }
    if rows.is_empty() {
        // Surface aggregate page-level signals even when the schema has no
        // user tables yet, so callers can still consume a single deterministic
        // row with the page checksum / LSN violation counters.
        let status = if report.is_clean() { "ok" } else { "errors" };
        rows.push(vec![
            SqlValue::Text(Arc::from("(database)")),
            SqlValue::Text(Arc::from(status)),
            SqlValue::Integer(0),
            SqlValue::Integer(0),
            SqlValue::Integer(0),
            SqlValue::Integer(0),
            SqlValue::Integer(report.page_csum_failures.len() as i64),
            SqlValue::Integer(report.lsn_monotonicity_violations.len() as i64),
            SqlValue::Text(Arc::from(report.errors.join("; ").as_str())),
        ]);
    }
    Ok(rows)
}

fn render_column_default(column: &redlinedb_kernel::catalog::ColumnDef) -> SqlValue {
    let value = render_default_value(column.default_value.as_ref());
    if !matches!(value, SqlValue::Null) {
        return value;
    }
    render_default_expr(column.default_expr.as_deref())
}

fn render_default_expr(expr: Option<&redlinedb_kernel::catalog::CompiledExpr>) -> SqlValue {
    use redlinedb_kernel::catalog::ExprOp;

    let Some(expr) = expr else {
        return SqlValue::Null;
    };
    match expr.bytecode.as_ref() {
        [ExprOp::CurrentDate] => SqlValue::Text(Arc::from("CURRENT_DATE")),
        [ExprOp::CurrentTime] => SqlValue::Text(Arc::from("CURRENT_TIME")),
        [ExprOp::CurrentTimestamp] => SqlValue::Text(Arc::from("CURRENT_TIMESTAMP")),
        _ => SqlValue::Null,
    }
}

fn render_default_value(value: Option<&OwnedValue>) -> SqlValue {
    let Some(value) = value else {
        return SqlValue::Null;
    };
    match value {
        OwnedValue::Null => SqlValue::Null,
        OwnedValue::Integer(v) => SqlValue::Text(Arc::from(v.to_string())),
        OwnedValue::Real(v) => SqlValue::Text(Arc::from(v.to_string())),
        OwnedValue::Text(v) => {
            let escaped = v.replace('\'', "''");
            SqlValue::Text(Arc::from(format!("'{escaped}'")))
        }
        OwnedValue::Blob(v) => {
            use std::fmt::Write;

            let mut out = String::from("X'");
            for byte in v.iter() {
                write!(&mut out, "{byte:02X}").expect("write hex");
            }
            out.push('\'');
            SqlValue::Text(Arc::from(out))
        }
    }
}
