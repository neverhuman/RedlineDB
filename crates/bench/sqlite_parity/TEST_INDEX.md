# SQLite Parity Test Index

Total cases: **1127**. Existing curated cases: **227**. Generated deterministic matrix cases: **900**.

## Profile/Priority counts

| Profile | Priority | Count |
|---|---:|---:|
| catalog | P4 | 15 |
| external_app | P4 | 2 |
| memory | P0 | 130 |
| memory | P1 | 572 |
| memory | P2 | 364 |
| memory | P3 | 18 |
| memory | P4 | 2 |
| side_effect | P4 | 3 |
| tempfile | P1 | 7 |
| tempfile | P2 | 6 |
| tempfile | P3 | 6 |
| tempfile | P4 | 2 |

## Cases

| ID | Priority | Profile | Category | Name | Description |
|---:|---|---|---|---|---|
| 0001 | P0 | memory | SQL_SELECT | `SELECT_CORE_EXPRESSIONS` | SELECT, arithmetic, concatenation, integer/real division, modulo, unary operators. |
| 0002 | P0 | memory | SQL_EXPRESSIONS | `LITERALS_AND_TYPEOF` | NULL, integer, real, text, blob literals, hex blobs, quote(). |
| 0003 | P0 | memory | SQL_DDL_DML | `CREATE_TABLE_INSERT_SELECT` | Basic CREATE TABLE, multi-row INSERT, SELECT aggregate count/group_concat. |
| 0004 | P0 | memory | SQL_CONSTRAINTS | `TABLE_CONSTRAINTS_SUCCESS` | PRIMARY KEY, UNIQUE, NOT NULL, CHECK, DEFAULT success path. |
| 0005 | P0 | memory | SQL_CONSTRAINTS_NEGATIVE | `UNIQUE_CONSTRAINT_FAILURE` | UNIQUE constraint failure behavior and CLI non-zero exit. |
| 0006 | P0 | memory | SQL_ROWID | `ROWID_INTEGER_PRIMARY_KEY` | ROWID aliasing through INTEGER PRIMARY KEY. |
| 0007 | P0 | memory | SQL_ROWID | `WITHOUT_ROWID_TABLE` | WITHOUT ROWID table creation and schema persistence. |
| 0008 | P0 | memory | SQL_DDL | `STRICT_TABLE` | STRICT table creation and strict type storage for valid values. |
| 0009 | P0 | memory | SQL_DDL_NEGATIVE | `STRICT_TABLE_TYPE_FAILURE` | STRICT table rejects invalid storage class. |
| 0010 | P0 | memory | SQL_DDL | `GENERATED_COLUMNS` | STORED and VIRTUAL generated columns. |
| 0011 | P0 | memory | SQL_DDL | `CREATE_TABLE_AS_SELECT` | CREATE TABLE AS SELECT and inferred storage classes. |
| 0012 | P0 | memory | SQL_INSERT | `INSERT_DEFAULT_VALUES` | INSERT DEFAULT VALUES with DEFAULT expressions. |
| 0013 | P0 | memory | SQL_INSERT | `INSERT_SELECT` | INSERT INTO ... SELECT. |
| 0014 | P0 | memory | SQL_INSERT | `INSERT_RETURNING` | INSERT ... RETURNING projection. |
| 0015 | P0 | memory | SQL_UPDATE | `UPDATE_BASIC` | UPDATE with WHERE and subsequent SELECT. |
| 0016 | P0 | memory | SQL_UPDATE | `UPDATE_RETURNING` | UPDATE ... RETURNING. |
| 0017 | P0 | memory | SQL_DELETE | `DELETE_BASIC` | DELETE with WHERE. |
| 0018 | P0 | memory | SQL_DELETE | `DELETE_RETURNING` | DELETE ... RETURNING. |
| 0019 | P0 | memory | SQL_REPLACE | `REPLACE_INTO` | REPLACE INTO conflict behavior. |
| 0020 | P0 | memory | SQL_UPSERT | `UPSERT_DO_UPDATE` | INSERT ... ON CONFLICT DO UPDATE. |
| 0021 | P0 | memory | SQL_UPSERT | `UPSERT_DO_NOTHING` | INSERT ... ON CONFLICT DO NOTHING. |
| 0022 | P0 | memory | SQL_TRANSACTION | `TRANSACTION_COMMIT` | BEGIN/COMMIT transaction persists changes inside connection. |
| 0023 | P0 | memory | SQL_TRANSACTION | `TRANSACTION_ROLLBACK` | ROLLBACK removes transactional changes. |
| 0024 | P0 | memory | SQL_SAVEPOINT | `SAVEPOINT_ROLLBACK_RELEASE` | SAVEPOINT, ROLLBACK TO, RELEASE nested savepoint behavior. |
| 0025 | P0 | memory | SQL_TRANSACTION | `BEGIN_MODES` | BEGIN DEFERRED, IMMEDIATE, EXCLUSIVE. |
| 0026 | P0 | memory | SQL_FOREIGN_KEYS | `FOREIGN_KEYS_CASCADE` | Foreign keys ON, ON UPDATE CASCADE, ON DELETE CASCADE. |
| 0027 | P0 | memory | SQL_FOREIGN_KEYS_NEGATIVE | `FOREIGN_KEY_FAILURE` | Foreign-key violation exits non-zero when foreign_keys is ON. |
| 0028 | P0 | memory | SQL_ALTER | `ALTER_TABLE_RENAME` | ALTER TABLE RENAME TO. |
| 0029 | P0 | memory | SQL_ALTER | `ALTER_TABLE_RENAME_COLUMN` | ALTER TABLE RENAME COLUMN. |
| 0030 | P0 | memory | SQL_ALTER | `ALTER_TABLE_ADD_DROP_COLUMN` | ALTER TABLE ADD COLUMN and DROP COLUMN. |
| 0031 | P0 | memory | SQL_INDEX | `CREATE_INDEX` | CREATE INDEX and schema visibility. |
| 0032 | P0 | memory | SQL_INDEX_NEGATIVE | `UNIQUE_INDEX_FAILURE` | UNIQUE INDEX enforcement failure. |
| 0033 | P0 | memory | SQL_INDEX | `PARTIAL_INDEX` | Partial index WHERE clause stored in schema. |
| 0034 | P0 | memory | SQL_INDEX | `EXPRESSION_INDEX` | Expression index on lower(name). |
| 0035 | P0 | memory | SQL_DROP | `DROP_INDEX` | DROP INDEX removes index from schema. |
| 0036 | P0 | memory | SQL_VIEW | `CREATE_VIEW` | CREATE VIEW and read-through SELECT. |
| 0037 | P0 | memory | SQL_DROP | `DROP_VIEW` | DROP VIEW removes view from schema. |
| 0038 | P0 | memory | SQL_TRIGGER | `CREATE_TRIGGER_AFTER` | AFTER INSERT trigger with NEW pseudo-table. |
| 0039 | P0 | memory | SQL_TRIGGER | `CREATE_TRIGGER_BEFORE` | BEFORE INSERT trigger with RAISE(IGNORE). |
| 0040 | P0 | memory | SQL_TRIGGER | `INSTEAD_OF_TRIGGER_ON_VIEW` | INSTEAD OF INSERT trigger on a view. |
| 0041 | P0 | memory | SQL_DROP | `DROP_TRIGGER` | DROP TRIGGER removes trigger from schema. |
| 0042 | P0 | memory | SQL_TEMP | `TEMP_TABLE_TEMP_SCHEMA` | CREATE TEMP TABLE and sqlite_temp_schema visibility. |
| 0043 | P0 | memory | SQL_ATTACH | `ATTACH_DETACH_MEMORY` | ATTACH ':memory:' AS aux and DETACH. |
| 0044 | P0 | memory | SQL_ANALYZE | `ANALYZE_SQLITE_STAT1` | ANALYZE creates sqlite_stat1 for indexed data. |
| 0045 | P0 | memory | SQL_REINDEX | `REINDEX_COMMAND` | REINDEX executes after index creation. |
| 0046 | P0 | memory | SQL_VACUUM | `VACUUM_MEMORY` | VACUUM on in-memory database plus integrity_check. |
| 0047 | P0 | memory | SQL_PRAGMA | `PRAGMA_FOREIGN_KEYS` | PRAGMA foreign_keys set/query. |
| 0048 | P0 | memory | SQL_PRAGMA | `PRAGMA_USER_VERSION` | PRAGMA user_version set/query in memory. |
| 0049 | P0 | memory | SQL_PRAGMA | `PRAGMA_TEMP_STORE_MEMORY` | PRAGMA temp_store MEMORY. |
| 0050 | P0 | memory | SQL_PRAGMA | `PRAGMA_TABLE_INFO_FUNCTION` | Table-valued PRAGMA function pragma_table_info(). |
| 0051 | P0 | memory | SQL_PRAGMA | `PRAGMA_INDEX_LIST_FUNCTION` | Table-valued PRAGMA function pragma_index_list(). |
| 0052 | P0 | memory | SQL_PRAGMA | `PRAGMA_INTEGRITY_QUICK_CHECK` | PRAGMA integrity_check and quick_check. |
| 0053 | P0 | memory | SQL_SELECT | `SELECT_WHERE_ORDER_LIMIT_OFFSET` | WHERE, ORDER BY, LIMIT, OFFSET. |
| 0054 | P0 | memory | SQL_JOIN | `JOINS_INNER_LEFT_CROSS_NATURAL` | INNER, LEFT, CROSS, NATURAL joins. |
| 0055 | P0 | memory | SQL_JOIN | `JOINS_RIGHT_FULL_OUTER` | RIGHT JOIN and FULL OUTER JOIN. |
| 0056 | P0 | memory | SQL_SELECT | `SUBQUERIES_EXISTS_IN` | Scalar subquery, EXISTS, IN. |
| 0057 | P0 | memory | SQL_SELECT | `COMPOUND_SELECT_UNION_INTERSECT_EXCEPT` | UNION, UNION ALL, INTERSECT, EXCEPT. |
| 0058 | P0 | memory | SQL_AGGREGATE | `GROUP_BY_HAVING` | GROUP BY and HAVING. |
| 0059 | P0 | memory | SQL_FUNCTIONS | `AGGREGATE_FUNCTIONS_CORE` | count, sum, total, avg, min, max, group_concat. |
| 0060 | P0 | memory | SQL_AGGREGATE | `FILTER_CLAUSE` | Aggregate FILTER clause. |
| 0061 | P0 | memory | SQL_WINDOW | `WINDOW_ROW_NUMBER_RANK` | row_number, rank, dense_rank window functions. |
| 0062 | P0 | memory | SQL_WINDOW | `WINDOW_FRAMES_ROWS` | Window frame ROWS BETWEEN 1 PRECEDING AND CURRENT ROW. |
| 0063 | P0 | memory | SQL_WINDOW | `WINDOW_EXCLUDE_CURRENT_ROW` | Window EXCLUDE CURRENT ROW. |
| 0064 | P0 | memory | SQL_CTE | `CTE_NON_RECURSIVE` | WITH non-recursive common table expression. |
| 0065 | P0 | memory | SQL_CTE | `CTE_RECURSIVE` | WITH RECURSIVE sequence generation. |
| 0066 | P0 | memory | SQL_VALUES | `VALUES_STATEMENT` | VALUES as a standalone statement. |
| 0067 | P0 | memory | SQL_EXPRESSIONS | `CASE_COALESCE_NULLIF_IIF` | CASE, coalesce, ifnull, nullif, iif/if spelling. |
| 0068 | P0 | memory | SQL_EXPRESSIONS | `CAST_AND_TYPE_AFFINITY` | CAST and storage affinity conversions. |
| 0069 | P0 | memory | SQL_COLLATION | `COLLATE_NOCASE_RTRIM_BINARY` | NOCASE, RTRIM, BINARY collation behavior. |
| 0070 | P0 | memory | SQL_OPERATORS | `LIKE_GLOB_MATCH_ESCAPE` | LIKE, GLOB, MATCH-style ESCAPE for LIKE. |
| 0071 | P0 | memory | SQL_OPERATORS | `BETWEEN_IN_ISNULL_IS` | BETWEEN, IN, IS NULL, IS / IS NOT. |
| 0072 | P0 | memory | SQL_SELECT | `ORDER_BY_NULLS_FIRST_LAST` | ORDER BY NULLS FIRST/LAST. |
| 0073 | P0 | memory | SQL_INDEX | `INDEXED_BY` | INDEXED BY clause. |
| 0074 | P0 | memory | SQL_INDEX | `NOT_INDEXED` | NOT INDEXED table scan clause. |
| 0075 | P0 | memory | SQL_EXPLAIN | `EXPLAIN_QUERY_PLAN` | EXPLAIN QUERY PLAN emits a query plan. |
| 0076 | P0 | memory | SQL_EXPLAIN | `EXPLAIN_BYTECODE` | EXPLAIN emits virtual-machine bytecode columns/opcodes. |
| 0077 | P0 | memory | CLI_SQL_INPUT | `COMMENTS_AND_CLI_TERMINATORS` | SQL comments plus CLI GO and slash statement terminators. |
| 0078 | P0 | memory | SQL_CONFLICT | `ON_CONFLICT_ALGORITHMS` | INSERT OR IGNORE and INSERT OR REPLACE conflict algorithms. |
| 0079 | P0 | memory | SQL_CONSTRAINTS_NEGATIVE | `CHECK_CONSTRAINT_FAILURE` | CHECK constraint failure. |
| 0080 | P0 | memory | SQL_CONSTRAINTS_NEGATIVE | `NOT_NULL_FAILURE` | NOT NULL constraint failure. |
| 0081 | P0 | memory | SQL_FUNCTIONS | `BLOBS_HEX_ZEROBLOB` | BLOB literal, hex(), zeroblob(). |
| 0082 | P0 | memory | SQL_FUNCTIONS | `CORE_STRING_FUNCTIONS` | substr, replace, upper, lower, trim, instr. |
| 0083 | P0 | memory | SQL_FUNCTIONS | `CORE_NUMERIC_FUNCTIONS` | abs, round, sign, min/max scalar. |
| 0084 | P0 | memory | SQL_FUNCTIONS | `CORE_FORMAT_QUOTE_HEX` | printf/format, quote, hex. |
| 0085 | P0 | memory | SQL_FUNCTIONS | `CORE_RANDOM_SHAPE` | random/randomblob shape without depending on random values. |
| 0086 | P0 | memory | SQL_FUNCTIONS | `DATE_TIME_FUNCTIONS` | date, time, datetime, unixepoch, strftime on fixed inputs. |
| 0087 | P0 | memory | SQL_FUNCTIONS | `DATE_TIMEDIFF_FUNCTION` | timediff() fixed input shape. |
| 0088 | P0 | memory | SQL_JSON | `JSON_SCALAR_FUNCTIONS` | json_valid, json_extract, json_type, json_array_length. |
| 0089 | P0 | memory | SQL_JSON | `JSON_TABLE_VALUED_FUNCTIONS` | json_each table-valued function. |
| 0090 | P0 | memory | SQL_JSON | `JSON_MUTATION_FUNCTIONS` | json_set, json_remove, json_patch. |
| 0091 | P2 | memory | SQL_FUNCTIONS_OPTIONAL | `MATH_FUNCTIONS_OPTIONAL` | Math functions when compiled/enabled: sin, pow, sqrt, ceil, floor. |
| 0092 | P3 | memory | SQL_FUNCTIONS_OPTIONAL | `PERCENTILE_FUNCTIONS_OPTIONAL` | Requires `SQLITE_PERCENTILE_FUNCTIONS`; percentile/median aggregate extension when compiled/enabled. |
| 0093 | P1 | memory | SQL_VIRTUAL_TABLE_OPTIONAL | `CREATE_VIRTUAL_TABLE_FTS5_OPTIONAL` | CREATE VIRTUAL TABLE USING fts5 and MATCH. |
| 0094 | P2 | memory | SQL_VIRTUAL_TABLE_OPTIONAL | `FTS5_HIGHLIGHT_OPTIONAL` | FTS5 highlight() auxiliary function. |
| 0095 | P2 | memory | SQL_VIRTUAL_TABLE_OPTIONAL | `CREATE_VIRTUAL_TABLE_RTREE_OPTIONAL` | CREATE VIRTUAL TABLE USING rtree. |
| 0096 | P3 | memory | SQL_VIRTUAL_TABLE_OPTIONAL | `DBSTAT_OPTIONAL` | dbstat virtual table when compiled/enabled. |
| 0097 | P2 | memory | CLI_EXTENSION_OPTIONAL | `CLI_GENERATE_SERIES_OPTIONAL` | CLI-bundled generate_series() table-valued function. |
| 0098 | P2 | memory | CLI_EXTENSION_OPTIONAL | `CLI_REGEXP_OPTIONAL` | CLI-bundled REGEXP operator support. |
| 0099 | P3 | memory | CLI_EXTENSION_OPTIONAL | `CLI_UINT_COLLATION_OPTIONAL` | CLI-bundled UINT collation. |
| 0100 | P0 | memory | SQL_SCHEMA | `SCHEMA_SQLITE_SCHEMA` | sqlite_schema introspection. |
| 0101 | P0 | memory | SQL_SCHEMA | `SCHEMA_SQLITE_MASTER_ALIAS` | sqlite_master compatibility alias. |
| 0102 | P0 | memory | SQL_CTE | `WITH_MATERIALIZED_HINTS` | AS MATERIALIZED and AS NOT MATERIALIZED CTE hints. |
| 0103 | P0 | memory | SQL_WINDOW | `WINDOW_NAMED_WINDOW` | Named WINDOW clause. |
| 0104 | P0 | memory | SQL_SELECT | `SELECT_DISTINCT` | SELECT DISTINCT duplicate elimination. |
| 0105 | P2 | memory | SQL_PRAGMA | `CASE_SENSITIVE_LIKE_PRAGMA` | PRAGMA case_sensitive_like toggle. |
| 0106 | P0 | memory | CLI_DOT_COMMAND | `DOT_HELP` | .help command list smoke check. |
| 0107 | P0 | memory | CLI_DOT_COMMAND | `DOT_HELP_PATTERN` | .help TOPIC for mode. |
| 0108 | P0 | memory | CLI_DOT_COMMAND | `DOT_MODE_CSV_AND_QUOTE` | .mode csv and .mode quote output formats. |
| 0109 | P0 | memory | CLI_DOT_COMMAND | `DOT_MODE_JSON` | .mode json output. |
| 0110 | P0 | memory | CLI_DOT_COMMAND | `DOT_MODE_LINE_COLUMN_TABLE_BOX_MARKDOWN` | .mode line/column/table/box/markdown smoke. |
| 0111 | P0 | memory | CLI_DOT_COMMAND | `DOT_HEADERS` | .headers on/off. |
| 0112 | P0 | memory | CLI_DOT_COMMAND | `DOT_SEPARATOR` | .separator column separator. |
| 0113 | P0 | memory | CLI_DOT_COMMAND | `DOT_NULLVALUE` | .nullvalue rendering. |
| 0114 | P0 | memory | CLI_DOT_COMMAND | `DOT_PRINT` | .print literal output. |
| 0115 | P0 | memory | CLI_DOT_COMMAND | `DOT_SCHEMA_TABLES_INDEXES` | .schema, .tables, .indexes. |
| 0116 | P0 | memory | CLI_DOT_COMMAND | `DOT_DATABASES` | .databases on in-memory database. |
| 0117 | P0 | memory | CLI_DOT_COMMAND | `DOT_DUMP` | .dump renders SQL for content. |
| 0118 | P0 | memory | CLI_DOT_COMMAND | `DOT_FULLSCHEMA` | .fullschema includes schema. |
| 0119 | P0 | memory | CLI_DOT_COMMAND | `DOT_EQP` | .eqp on emits query plan. |
| 0120 | P0 | memory | CLI_DOT_COMMAND | `DOT_EXPLAIN` | .explain on formats EXPLAIN output. |
| 0121 | P0 | memory | CLI_DOT_COMMAND | `DOT_PARAMETER` | .parameter init/set/list/clear and named parameter binding. |
| 0122 | P0 | memory | CLI_DOT_COMMAND | `DOT_CHANGES` | .changes on/off. |
| 0123 | P0 | memory | CLI_DOT_COMMAND | `DOT_ECHO` | .echo on echoes input. |
| 0124 | P0 | memory | CLI_DOT_COMMAND_NEGATIVE | `DOT_BAIL_OFF` | .bail off allows later statements after error. |
| 0125 | P3 | memory | CLI_DOT_COMMAND_DIAGNOSTIC | `DOT_TIMER` | .timer on emits timing diagnostics. |
| 0126 | P3 | memory | CLI_DOT_COMMAND_DIAGNOSTIC | `DOT_STATS` | .stats on emits runtime statistics. |
| 0127 | P0 | memory | CLI_DOT_COMMAND | `DOT_LIMIT` | .limit query/change SQLITE_LIMIT. |
| 0128 | P0 | memory | CLI_DOT_COMMAND | `DOT_DBCONFIG` | .dbconfig set/query smoke. |
| 0129 | P0 | memory | CLI_DOT_COMMAND | `DOT_CONNECTION` | .connection open/switch connections. |
| 0130 | P0 | memory | CLI_DOT_COMMAND | `DOT_OPEN_MEMORY` | .open :memory: no persistent DB. |
| 0131 | P0 | memory | CLI_DOT_COMMAND | `DOT_TIMEOUT` | .timeout set busy timeout. |
| 0132 | P2 | memory | CLI_DOT_COMMAND_DIAGNOSTIC | `DOT_TRACE_STDOUT` | .trace stdout emits SQL trace. |
| 0133 | P3 | memory | CLI_DOT_COMMAND_DIAGNOSTIC | `DOT_AUTH` | .auth on/off authorizer callback display. |
| 0134 | P3 | memory | CLI_DOT_COMMAND_DIAGNOSTIC | `DOT_CRLF` | Requires `CLI_CRLF_COMMAND`; .crlf on/off with normalized line endings; diagnostic on SQLite 3.45.1 shells that do not support `.crlf`. |
| 0135 | P0 | memory | CLI_DOT_COMMAND | `DOT_PROGRESS` | .progress handler smoke. |
| 0136 | P0 | memory | CLI_DOT_COMMAND | `DOT_LOG` | .log stdout/on/off smoke. |
| 0137 | P2 | memory | CLI_DOT_COMMAND_DIAGNOSTIC | `DOT_VERSION` | .version version output smoke. |
| 0138 | P3 | memory | CLI_DOT_COMMAND_DIAGNOSTIC | `DOT_VFSNAME_LIST_INFO` | .vfsname, .vfslist, .vfsinfo smoke. |
| 0139 | P3 | memory | CLI_DOT_COMMAND_DIAGNOSTIC | `DOT_LINT_FKEY_INDEXES` | .lint fkey-indexes smoke. |
| 0140 | P3 | memory | CLI_DOT_COMMAND_OPTIONAL | `DOT_EXPERT_OPTIONAL` | .expert index recommendation smoke. |
| 0141 | P0 | memory | CLI_DOT_COMMAND | `DOT_SHA3SUM` | .sha3sum database content hash shape. |
| 0142 | P0 | memory | CLI_DOT_COMMAND | `DOT_EXIT_CODE` | .exit CODE returns that process code. |
| 0143 | P0 | memory | CLI_DOT_COMMAND | `DOT_QUIT` | .quit stops input interpretation. |
| 0144 | P0 | memory | CLI_DOT_COMMAND | `DOT_PROMPT` | .prompt smoke in batch mode. |
| 0145 | P3 | memory | CLI_DOT_COMMAND_DIAGNOSTIC | `DOT_SCANSTATS` | .scanstats smoke when available. |
| 0146 | P1 | tempfile | CLI_TEMPFILE | `DOT_READ_TEMPFILE` | .read from a short-lived temp SQL file. |
| 0147 | P1 | tempfile | CLI_TEMPFILE | `DOT_IMPORT_CSV_TEMPFILE` | .import CSV from a short-lived temp file. |
| 0148 | P1 | tempfile | CLI_TEMPFILE | `DOT_OUTPUT_TEMPFILE` | .output writes next results to temp file then readfile verifies. |
| 0149 | P1 | tempfile | CLI_TEMPFILE | `DOT_ONCE_TEMPFILE` | .once writes one statement to temp file only. |
| 0150 | P1 | tempfile | CLI_TEMPFILE | `DOT_BACKUP_RESTORE_TEMPFILE` | .backup and .restore via short-lived temp database file. |
| 0151 | P1 | tempfile | CLI_TEMPFILE | `DOT_SAVE_RESTORE_TEMPFILE` | .save alias for backup, then restore. |
| 0152 | P2 | tempfile | CLI_TEMPFILE | `DOT_CLONE_TEMPFILE` | .clone into short-lived temp database file. |
| 0153 | P2 | tempfile | CLI_TEMPFILE | `DOT_CD_TEMPFILE` | .cd into temp directory. |
| 0154 | P3 | tempfile | CLI_TEMPFILE_DIAGNOSTIC | `DOT_DBINFO_TEMPFILE` | Requires `CLI_DBINFO_COMMAND`; .dbinfo on temp database file; diagnostic when sqlite_dbpage support is unavailable. |
| 0155 | P3 | tempfile | CLI_TEMPFILE | `DOT_DBTOTXT_TEMPFILE` | Requires `CLI_DBTOTXT_COMMAND`; .dbtotxt hex dump shape. |
| 0156 | P3 | tempfile | CLI_TEMPFILE | `DOT_RECOVER_TEMPFILE` | Requires `CLI_RECOVER_COMMAND`; .recover on valid short-lived db file emits recovery SQL. |
| 0157 | P3 | tempfile | CLI_TEMPFILE_OPTIONAL | `DOT_ARCHIVE_TEMPFILE_OPTIONAL` | .archive create/list using short-lived files. |
| 0158 | P4 | side_effect | CLI_SIDE_EFFECT | `DOT_SHELL_SIDE_EFFECT` | .shell command is excluded by default; enable side-effect profile. |
| 0159 | P4 | side_effect | CLI_SIDE_EFFECT | `DOT_SYSTEM_SIDE_EFFECT` | .system command is excluded by default; enable side-effect profile. |
| 0160 | P4 | external_app | CLI_EXTERNAL_APP | `DOT_EXCEL_EXTERNAL_APP` | .excel opens spreadsheet app; catalog-only by default. |
| 0161 | P4 | external_app | CLI_EXTERNAL_APP | `DOT_WWW_EXTERNAL_APP` | .www opens browser; catalog-only by default. |
| 0162 | P4 | side_effect | CLI_SIDE_EFFECT | `DOT_LOAD_EXTENSION_NEGATIVE` | .load non-existent extension negative path. |
| 0163 | P4 | catalog | CLI_CATALOG | `DOT_FILECTRL_CATALOG` | .filectrl is build/VFS-specific; catalog entry intentionally skipped unless custom case is added. |
| 0164 | P4 | catalog | CLI_CATALOG | `DOT_IMPOSTER_CATALOG` | .imposter is unsafe/testing-oriented; catalog entry skipped by default. |
| 0165 | P4 | catalog | CLI_CATALOG | `DOT_INTCK_CATALOG` | .intck incremental integrity check is build/version-specific; catalog entry skipped by default. |
| 0166 | P4 | catalog | CLI_CATALOG | `DOT_SESSION_CATALOG` | .session requires session extension; catalog entry skipped by default. |
| 0167 | P4 | catalog | CLI_CATALOG | `DOT_UNMODULE_CATALOG` | .unmodule mutates registered virtual table modules; catalog entry skipped by default. |
| 0168 | P4 | catalog | CLI_CATALOG | `DOT_CHECK_CATALOG` | .check relies on shell test harness state; catalog entry skipped by default. |
| 0169 | P2 | memory | CLI_DOT_COMMAND | `DOT_NONCE_SAFE_MODE` | .nonce with --safe escape nonce for one command. |
| 0170 | P1 | memory | CLI_OPTION | `OPT_VERSION` | -version command-line option. |
| 0171 | P1 | memory | CLI_OPTION | `OPT_HELP` | -help command-line option. |
| 0172 | P1 | memory | CLI_OPTION | `OPT_CMD` | -cmd runs command before input. |
| 0173 | P1 | memory | CLI_OPTION | `OPT_LIST_MODE` | -list output mode. |
| 0174 | P1 | memory | CLI_OPTION | `OPT_CSV_MODE` | -csv output mode. |
| 0175 | P1 | memory | CLI_OPTION | `OPT_QUOTE_MODE` | -quote output mode. |
| 0176 | P2 | memory | CLI_OPTION | `OPT_LINE_MODE` | -line output mode. |
| 0177 | P1 | memory | CLI_OPTION | `OPT_JSON_MODE` | -json output mode. |
| 0178 | P2 | memory | CLI_OPTION | `OPT_HTML_MODE` | -html output mode. |
| 0179 | P2 | memory | CLI_OPTION | `OPT_MARKDOWN_MODE` | -markdown output mode. |
| 0180 | P2 | memory | CLI_OPTION | `OPT_BOX_MODE` | -box output mode smoke. |
| 0181 | P2 | memory | CLI_OPTION | `OPT_TABLE_MODE` | -table output mode smoke. |
| 0182 | P2 | memory | CLI_OPTION | `OPT_COLUMN_MODE` | -column output mode smoke. |
| 0183 | P2 | memory | CLI_OPTION | `OPT_TABS_MODE` | -tabs output mode. |
| 0184 | P2 | memory | CLI_OPTION | `OPT_ASCII_MODE` | -ascii output mode. |
| 0185 | P1 | memory | CLI_OPTION | `OPT_SEPARATOR` | -separator output separator. |
| 0186 | P2 | memory | CLI_OPTION | `OPT_NEWLINE` | -newline row separator. |
| 0187 | P1 | memory | CLI_OPTION | `OPT_NULLVALUE` | -nullvalue rendering. |
| 0188 | P1 | memory | CLI_OPTION | `OPT_HEADER` | -header / -noheader. |
| 0189 | P2 | memory | CLI_OPTION | `OPT_ECHO` | -echo echoes input. |
| 0190 | P1 | memory | CLI_OPTION_NEGATIVE | `OPT_BAIL` | -bail exits after first SQL error. |
| 0191 | P2 | memory | CLI_OPTION | `OPT_BATCH` | -batch smoke. |
| 0192 | P2 | tempfile | CLI_OPTION_TEMPFILE | `OPT_INIT_TEMPFILE` | -init reads temp init script. |
| 0193 | P2 | tempfile | CLI_OPTION_TEMPFILE | `OPT_READONLY_TEMPFILE` | -readonly opens temp db read-only. |
| 0194 | P3 | tempfile | CLI_OPTION_TEMPFILE_DIAGNOSTIC | `OPT_IFEXISTS_NEGATIVE_TEMPFILE` | -ifexists refuses missing temp db; diagnostic on SQLite 3.45.1 shells without -ifexists. |
| 0195 | P2 | memory | CLI_OPTION_NEGATIVE | `OPT_SAFE_MODE_BLOCKS_SHELL` | -safe blocks unsafe shell command. |
| 0196 | P3 | memory | CLI_OPTION | `OPT_MMAP` | -mmap smoke. |
| 0197 | P3 | tempfile | CLI_OPTION_TEMPFILE | `OPT_MAXSIZE_DESERIALIZE_TEMPFILE` | -deserialize/-maxsize smoke with temp db. |
| 0198 | P3 | memory | CLI_OPTION | `OPT_LOOKASIDE` | -lookaside smoke. |
| 0199 | P3 | memory | CLI_OPTION | `OPT_PAGECACHE` | -pagecache smoke. |
| 0200 | P4 | memory | CLI_OPTION | `OPT_HEAP` | -heap smoke. |
| 0201 | P4 | memory | CLI_OPTION | `OPT_NO_ROWID_IN_VIEW` | -no-rowid-in-view smoke. |
| 0202 | P4 | tempfile | CLI_OPTION_TEMPFILE | `OPT_APPEND_TEMPFILE` | -append option smoke with short-lived file. |
| 0203 | P4 | tempfile | CLI_OPTION_TEMPFILE_OPTIONAL | `OPT_ARCHIVE_A_TEMPFILE_OPTIONAL` | -A archive command-line option smoke. |
| 0204 | P4 | catalog | CLI_OPTION_CATALOG | `OPT_ZIP_TEMPFILE_CATALOG` | -zip opens ZIP archive; catalog-only unless a zip fixture is added. |
| 0205 | P4 | catalog | CLI_OPTION_CATALOG | `OPT_VFS_CATALOG` | -vfs is platform-specific; catalog-only by default. |
| 0206 | P4 | catalog | CLI_OPTION_CATALOG | `OPT_MEMTRACE_CATALOG` | -memtrace diagnostic option; catalog-only by default. |
| 0207 | P4 | catalog | CLI_OPTION_CATALOG | `OPT_PCACHETRACE_CATALOG` | -pcachetrace diagnostic option; catalog-only by default. |
| 0208 | P4 | catalog | CLI_OPTION_CATALOG | `OPT_VFSTRACE_CATALOG` | -vfstrace diagnostic option; catalog-only by default. |
| 0209 | P4 | catalog | CLI_OPTION_CATALOG | `OPT_INTERACTIVE_CATALOG` | -interactive requires terminal behavior; catalog-only by default. |
| 0210 | P4 | catalog | CLI_OPTION_CATALOG | `OPT_NOUNICODE_UTF8_CATALOG` | -utf8 / -no-utf8 are Windows console compatibility no-ops; catalog-only by default. |
| 0211 | P1 | tempfile | SQL_TEMPFILE | `SQL_ATTACH_TEMPFILE_DATABASE` | ATTACH a short-lived on-disk temp database path; no persistence after runner cleanup. |
| 0212 | P2 | tempfile | SQL_TEMPFILE | `SQL_VACUUM_INTO_TEMPFILE` | VACUUM INTO short-lived temp database file. |
| 0213 | P2 | tempfile | SQL_TEMPFILE | `SQL_WAL_CHECKPOINT_TEMPFILE` | PRAGMA journal_mode=WAL and wal_checkpoint on temp database file. |
| 0214 | P0 | memory | SQL_DROP | `DROP_TABLE` | DROP TABLE removes table from schema. |
| 0215 | P0 | memory | SQL_TRANSACTION | `TRANSACTION_SYNONYMS_END_COMMIT_TRANSACTION` | BEGIN TRANSACTION, END TRANSACTION, COMMIT TRANSACTION synonyms. |
| 0216 | P0 | memory | SQL_TRANSACTION | `ROLLBACK_TRANSACTION_SYNTAX` | ROLLBACK TRANSACTION syntax. |
| 0217 | P0 | memory | SQL_ATTACH | `DETACH_DATABASE_SYNTAX` | DETACH DATABASE syntax after ATTACH DATABASE. |
| 0218 | P0 | memory | SQL_PRAGMA | `PRAGMA_FORMS_SCHEMA_EQUALS_PARENS` | PRAGMA schema prefix, equals syntax, and parenthesized syntax. |
| 0219 | P3 | memory | SQL_UPDATE_OPTIONAL | `UPDATE_LIMIT_OPTIONAL` | UPDATE ... ORDER BY ... LIMIT when compiled with update/delete limit support. |
| 0220 | P3 | memory | SQL_DELETE_OPTIONAL | `DELETE_LIMIT_OPTIONAL` | DELETE ... ORDER BY ... LIMIT when compiled with update/delete limit support. |
| 0221 | P2 | memory | CLI_OPTION | `OPT_DOUBLE_DASH_END_OPTIONS` | -- ends option parsing. |
| 0222 | P3 | memory | CLI_OPTION | `OPT_ESCAPE_SYMBOL` | Requires `CLI_ESCAPE_SYMBOL_OPTION`; -escape symbol renders control characters with symbolic escapes. |
| 0223 | P2 | memory | CLI_OPTION | `OPT_NOHEADER` | -noheader overrides header output. |
| 0224 | P3 | memory | CLI_OPTION_DIAGNOSTIC | `OPT_STATS` | -stats command-line option emits statistics. |
| 0225 | P2 | memory | CLI_OPTION | `OPT_NONCE_SAFE_MODE` | -nonce with --safe allows one matching .nonce escape. |
| 0226 | P4 | catalog | CLI_OPTION_CATALOG | `OPT_NOFOLLOW_CATALOG` | -nofollow symlink behavior is platform-specific; catalog-only. |
| 0227 | P4 | catalog | CLI_OPTION_CATALOG | `OPT_UNSAFE_TESTING_CATALOG` | -unsafe-testing enables dangerous test controls; catalog-only. |
| 0228 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_ARITH_001` | Generated deterministic SQLite parity case for SCALAR_ARITH_001. |
| 0229 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_CAST_TYPEOF_001` | Generated deterministic SQLite parity case for SCALAR_CAST_TYPEOF_001. |
| 0230 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_STRING_001` | Generated deterministic SQLite parity case for SCALAR_STRING_001. |
| 0231 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_NULL_COALESCE_001` | Generated deterministic SQLite parity case for SCALAR_NULL_COALESCE_001. |
| 0232 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_ARITH_002` | Generated deterministic SQLite parity case for SCALAR_ARITH_002. |
| 0233 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_CAST_TYPEOF_002` | Generated deterministic SQLite parity case for SCALAR_CAST_TYPEOF_002. |
| 0234 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_STRING_002` | Generated deterministic SQLite parity case for SCALAR_STRING_002. |
| 0235 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_NULL_COALESCE_002` | Generated deterministic SQLite parity case for SCALAR_NULL_COALESCE_002. |
| 0236 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_ARITH_003` | Generated deterministic SQLite parity case for SCALAR_ARITH_003. |
| 0237 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_CAST_TYPEOF_003` | Generated deterministic SQLite parity case for SCALAR_CAST_TYPEOF_003. |
| 0238 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_STRING_003` | Generated deterministic SQLite parity case for SCALAR_STRING_003. |
| 0239 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_NULL_COALESCE_003` | Generated deterministic SQLite parity case for SCALAR_NULL_COALESCE_003. |
| 0240 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_ARITH_004` | Generated deterministic SQLite parity case for SCALAR_ARITH_004. |
| 0241 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_CAST_TYPEOF_004` | Generated deterministic SQLite parity case for SCALAR_CAST_TYPEOF_004. |
| 0242 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_STRING_004` | Generated deterministic SQLite parity case for SCALAR_STRING_004. |
| 0243 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_NULL_COALESCE_004` | Generated deterministic SQLite parity case for SCALAR_NULL_COALESCE_004. |
| 0244 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_ARITH_005` | Generated deterministic SQLite parity case for SCALAR_ARITH_005. |
| 0245 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_CAST_TYPEOF_005` | Generated deterministic SQLite parity case for SCALAR_CAST_TYPEOF_005. |
| 0246 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_STRING_005` | Generated deterministic SQLite parity case for SCALAR_STRING_005. |
| 0247 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_NULL_COALESCE_005` | Generated deterministic SQLite parity case for SCALAR_NULL_COALESCE_005. |
| 0248 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_ARITH_006` | Generated deterministic SQLite parity case for SCALAR_ARITH_006. |
| 0249 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_CAST_TYPEOF_006` | Generated deterministic SQLite parity case for SCALAR_CAST_TYPEOF_006. |
| 0250 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_STRING_006` | Generated deterministic SQLite parity case for SCALAR_STRING_006. |
| 0251 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_NULL_COALESCE_006` | Generated deterministic SQLite parity case for SCALAR_NULL_COALESCE_006. |
| 0252 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_ARITH_007` | Generated deterministic SQLite parity case for SCALAR_ARITH_007. |
| 0253 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_CAST_TYPEOF_007` | Generated deterministic SQLite parity case for SCALAR_CAST_TYPEOF_007. |
| 0254 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_STRING_007` | Generated deterministic SQLite parity case for SCALAR_STRING_007. |
| 0255 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_NULL_COALESCE_007` | Generated deterministic SQLite parity case for SCALAR_NULL_COALESCE_007. |
| 0256 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_ARITH_008` | Generated deterministic SQLite parity case for SCALAR_ARITH_008. |
| 0257 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_CAST_TYPEOF_008` | Generated deterministic SQLite parity case for SCALAR_CAST_TYPEOF_008. |
| 0258 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_STRING_008` | Generated deterministic SQLite parity case for SCALAR_STRING_008. |
| 0259 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_NULL_COALESCE_008` | Generated deterministic SQLite parity case for SCALAR_NULL_COALESCE_008. |
| 0260 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_ARITH_009` | Generated deterministic SQLite parity case for SCALAR_ARITH_009. |
| 0261 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_CAST_TYPEOF_009` | Generated deterministic SQLite parity case for SCALAR_CAST_TYPEOF_009. |
| 0262 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_STRING_009` | Generated deterministic SQLite parity case for SCALAR_STRING_009. |
| 0263 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_NULL_COALESCE_009` | Generated deterministic SQLite parity case for SCALAR_NULL_COALESCE_009. |
| 0264 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_ARITH_010` | Generated deterministic SQLite parity case for SCALAR_ARITH_010. |
| 0265 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_CAST_TYPEOF_010` | Generated deterministic SQLite parity case for SCALAR_CAST_TYPEOF_010. |
| 0266 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_STRING_010` | Generated deterministic SQLite parity case for SCALAR_STRING_010. |
| 0267 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_NULL_COALESCE_010` | Generated deterministic SQLite parity case for SCALAR_NULL_COALESCE_010. |
| 0268 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_ARITH_011` | Generated deterministic SQLite parity case for SCALAR_ARITH_011. |
| 0269 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_CAST_TYPEOF_011` | Generated deterministic SQLite parity case for SCALAR_CAST_TYPEOF_011. |
| 0270 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_STRING_011` | Generated deterministic SQLite parity case for SCALAR_STRING_011. |
| 0271 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_NULL_COALESCE_011` | Generated deterministic SQLite parity case for SCALAR_NULL_COALESCE_011. |
| 0272 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_ARITH_012` | Generated deterministic SQLite parity case for SCALAR_ARITH_012. |
| 0273 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_CAST_TYPEOF_012` | Generated deterministic SQLite parity case for SCALAR_CAST_TYPEOF_012. |
| 0274 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_STRING_012` | Generated deterministic SQLite parity case for SCALAR_STRING_012. |
| 0275 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_NULL_COALESCE_012` | Generated deterministic SQLite parity case for SCALAR_NULL_COALESCE_012. |
| 0276 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_ARITH_013` | Generated deterministic SQLite parity case for SCALAR_ARITH_013. |
| 0277 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_CAST_TYPEOF_013` | Generated deterministic SQLite parity case for SCALAR_CAST_TYPEOF_013. |
| 0278 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_STRING_013` | Generated deterministic SQLite parity case for SCALAR_STRING_013. |
| 0279 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_NULL_COALESCE_013` | Generated deterministic SQLite parity case for SCALAR_NULL_COALESCE_013. |
| 0280 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_ARITH_014` | Generated deterministic SQLite parity case for SCALAR_ARITH_014. |
| 0281 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_CAST_TYPEOF_014` | Generated deterministic SQLite parity case for SCALAR_CAST_TYPEOF_014. |
| 0282 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_STRING_014` | Generated deterministic SQLite parity case for SCALAR_STRING_014. |
| 0283 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_NULL_COALESCE_014` | Generated deterministic SQLite parity case for SCALAR_NULL_COALESCE_014. |
| 0284 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_ARITH_015` | Generated deterministic SQLite parity case for SCALAR_ARITH_015. |
| 0285 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_CAST_TYPEOF_015` | Generated deterministic SQLite parity case for SCALAR_CAST_TYPEOF_015. |
| 0286 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_STRING_015` | Generated deterministic SQLite parity case for SCALAR_STRING_015. |
| 0287 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_NULL_COALESCE_015` | Generated deterministic SQLite parity case for SCALAR_NULL_COALESCE_015. |
| 0288 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_ARITH_016` | Generated deterministic SQLite parity case for SCALAR_ARITH_016. |
| 0289 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_CAST_TYPEOF_016` | Generated deterministic SQLite parity case for SCALAR_CAST_TYPEOF_016. |
| 0290 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_STRING_016` | Generated deterministic SQLite parity case for SCALAR_STRING_016. |
| 0291 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_NULL_COALESCE_016` | Generated deterministic SQLite parity case for SCALAR_NULL_COALESCE_016. |
| 0292 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_ARITH_017` | Generated deterministic SQLite parity case for SCALAR_ARITH_017. |
| 0293 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_CAST_TYPEOF_017` | Generated deterministic SQLite parity case for SCALAR_CAST_TYPEOF_017. |
| 0294 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_STRING_017` | Generated deterministic SQLite parity case for SCALAR_STRING_017. |
| 0295 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_NULL_COALESCE_017` | Generated deterministic SQLite parity case for SCALAR_NULL_COALESCE_017. |
| 0296 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_ARITH_018` | Generated deterministic SQLite parity case for SCALAR_ARITH_018. |
| 0297 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_CAST_TYPEOF_018` | Generated deterministic SQLite parity case for SCALAR_CAST_TYPEOF_018. |
| 0298 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_STRING_018` | Generated deterministic SQLite parity case for SCALAR_STRING_018. |
| 0299 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_NULL_COALESCE_018` | Generated deterministic SQLite parity case for SCALAR_NULL_COALESCE_018. |
| 0300 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_ARITH_019` | Generated deterministic SQLite parity case for SCALAR_ARITH_019. |
| 0301 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_CAST_TYPEOF_019` | Generated deterministic SQLite parity case for SCALAR_CAST_TYPEOF_019. |
| 0302 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_STRING_019` | Generated deterministic SQLite parity case for SCALAR_STRING_019. |
| 0303 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_NULL_COALESCE_019` | Generated deterministic SQLite parity case for SCALAR_NULL_COALESCE_019. |
| 0304 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_ARITH_020` | Generated deterministic SQLite parity case for SCALAR_ARITH_020. |
| 0305 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_CAST_TYPEOF_020` | Generated deterministic SQLite parity case for SCALAR_CAST_TYPEOF_020. |
| 0306 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_STRING_020` | Generated deterministic SQLite parity case for SCALAR_STRING_020. |
| 0307 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_NULL_COALESCE_020` | Generated deterministic SQLite parity case for SCALAR_NULL_COALESCE_020. |
| 0308 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_ARITH_021` | Generated deterministic SQLite parity case for SCALAR_ARITH_021. |
| 0309 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_CAST_TYPEOF_021` | Generated deterministic SQLite parity case for SCALAR_CAST_TYPEOF_021. |
| 0310 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_STRING_021` | Generated deterministic SQLite parity case for SCALAR_STRING_021. |
| 0311 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_NULL_COALESCE_021` | Generated deterministic SQLite parity case for SCALAR_NULL_COALESCE_021. |
| 0312 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_ARITH_022` | Generated deterministic SQLite parity case for SCALAR_ARITH_022. |
| 0313 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_CAST_TYPEOF_022` | Generated deterministic SQLite parity case for SCALAR_CAST_TYPEOF_022. |
| 0314 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_STRING_022` | Generated deterministic SQLite parity case for SCALAR_STRING_022. |
| 0315 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_NULL_COALESCE_022` | Generated deterministic SQLite parity case for SCALAR_NULL_COALESCE_022. |
| 0316 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_ARITH_023` | Generated deterministic SQLite parity case for SCALAR_ARITH_023. |
| 0317 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_CAST_TYPEOF_023` | Generated deterministic SQLite parity case for SCALAR_CAST_TYPEOF_023. |
| 0318 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_STRING_023` | Generated deterministic SQLite parity case for SCALAR_STRING_023. |
| 0319 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_NULL_COALESCE_023` | Generated deterministic SQLite parity case for SCALAR_NULL_COALESCE_023. |
| 0320 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_ARITH_024` | Generated deterministic SQLite parity case for SCALAR_ARITH_024. |
| 0321 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_CAST_TYPEOF_024` | Generated deterministic SQLite parity case for SCALAR_CAST_TYPEOF_024. |
| 0322 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_STRING_024` | Generated deterministic SQLite parity case for SCALAR_STRING_024. |
| 0323 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_NULL_COALESCE_024` | Generated deterministic SQLite parity case for SCALAR_NULL_COALESCE_024. |
| 0324 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_ARITH_025` | Generated deterministic SQLite parity case for SCALAR_ARITH_025. |
| 0325 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_CAST_TYPEOF_025` | Generated deterministic SQLite parity case for SCALAR_CAST_TYPEOF_025. |
| 0326 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_STRING_025` | Generated deterministic SQLite parity case for SCALAR_STRING_025. |
| 0327 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_NULL_COALESCE_025` | Generated deterministic SQLite parity case for SCALAR_NULL_COALESCE_025. |
| 0328 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_ARITH_026` | Generated deterministic SQLite parity case for SCALAR_ARITH_026. |
| 0329 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_CAST_TYPEOF_026` | Generated deterministic SQLite parity case for SCALAR_CAST_TYPEOF_026. |
| 0330 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_STRING_026` | Generated deterministic SQLite parity case for SCALAR_STRING_026. |
| 0331 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_NULL_COALESCE_026` | Generated deterministic SQLite parity case for SCALAR_NULL_COALESCE_026. |
| 0332 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_ARITH_027` | Generated deterministic SQLite parity case for SCALAR_ARITH_027. |
| 0333 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_CAST_TYPEOF_027` | Generated deterministic SQLite parity case for SCALAR_CAST_TYPEOF_027. |
| 0334 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_STRING_027` | Generated deterministic SQLite parity case for SCALAR_STRING_027. |
| 0335 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_NULL_COALESCE_027` | Generated deterministic SQLite parity case for SCALAR_NULL_COALESCE_027. |
| 0336 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_ARITH_028` | Generated deterministic SQLite parity case for SCALAR_ARITH_028. |
| 0337 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_CAST_TYPEOF_028` | Generated deterministic SQLite parity case for SCALAR_CAST_TYPEOF_028. |
| 0338 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_STRING_028` | Generated deterministic SQLite parity case for SCALAR_STRING_028. |
| 0339 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_NULL_COALESCE_028` | Generated deterministic SQLite parity case for SCALAR_NULL_COALESCE_028. |
| 0340 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_ARITH_029` | Generated deterministic SQLite parity case for SCALAR_ARITH_029. |
| 0341 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_CAST_TYPEOF_029` | Generated deterministic SQLite parity case for SCALAR_CAST_TYPEOF_029. |
| 0342 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_STRING_029` | Generated deterministic SQLite parity case for SCALAR_STRING_029. |
| 0343 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_NULL_COALESCE_029` | Generated deterministic SQLite parity case for SCALAR_NULL_COALESCE_029. |
| 0344 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_ARITH_030` | Generated deterministic SQLite parity case for SCALAR_ARITH_030. |
| 0345 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_CAST_TYPEOF_030` | Generated deterministic SQLite parity case for SCALAR_CAST_TYPEOF_030. |
| 0346 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_STRING_030` | Generated deterministic SQLite parity case for SCALAR_STRING_030. |
| 0347 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_NULL_COALESCE_030` | Generated deterministic SQLite parity case for SCALAR_NULL_COALESCE_030. |
| 0348 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_ARITH_031` | Generated deterministic SQLite parity case for SCALAR_ARITH_031. |
| 0349 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_CAST_TYPEOF_031` | Generated deterministic SQLite parity case for SCALAR_CAST_TYPEOF_031. |
| 0350 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_STRING_031` | Generated deterministic SQLite parity case for SCALAR_STRING_031. |
| 0351 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_NULL_COALESCE_031` | Generated deterministic SQLite parity case for SCALAR_NULL_COALESCE_031. |
| 0352 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_ARITH_032` | Generated deterministic SQLite parity case for SCALAR_ARITH_032. |
| 0353 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_CAST_TYPEOF_032` | Generated deterministic SQLite parity case for SCALAR_CAST_TYPEOF_032. |
| 0354 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_STRING_032` | Generated deterministic SQLite parity case for SCALAR_STRING_032. |
| 0355 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_NULL_COALESCE_032` | Generated deterministic SQLite parity case for SCALAR_NULL_COALESCE_032. |
| 0356 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_ARITH_033` | Generated deterministic SQLite parity case for SCALAR_ARITH_033. |
| 0357 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_CAST_TYPEOF_033` | Generated deterministic SQLite parity case for SCALAR_CAST_TYPEOF_033. |
| 0358 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_STRING_033` | Generated deterministic SQLite parity case for SCALAR_STRING_033. |
| 0359 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_NULL_COALESCE_033` | Generated deterministic SQLite parity case for SCALAR_NULL_COALESCE_033. |
| 0360 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_ARITH_034` | Generated deterministic SQLite parity case for SCALAR_ARITH_034. |
| 0361 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_CAST_TYPEOF_034` | Generated deterministic SQLite parity case for SCALAR_CAST_TYPEOF_034. |
| 0362 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_STRING_034` | Generated deterministic SQLite parity case for SCALAR_STRING_034. |
| 0363 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_NULL_COALESCE_034` | Generated deterministic SQLite parity case for SCALAR_NULL_COALESCE_034. |
| 0364 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_ARITH_035` | Generated deterministic SQLite parity case for SCALAR_ARITH_035. |
| 0365 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_CAST_TYPEOF_035` | Generated deterministic SQLite parity case for SCALAR_CAST_TYPEOF_035. |
| 0366 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_STRING_035` | Generated deterministic SQLite parity case for SCALAR_STRING_035. |
| 0367 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_NULL_COALESCE_035` | Generated deterministic SQLite parity case for SCALAR_NULL_COALESCE_035. |
| 0368 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_ARITH_036` | Generated deterministic SQLite parity case for SCALAR_ARITH_036. |
| 0369 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_CAST_TYPEOF_036` | Generated deterministic SQLite parity case for SCALAR_CAST_TYPEOF_036. |
| 0370 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_STRING_036` | Generated deterministic SQLite parity case for SCALAR_STRING_036. |
| 0371 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_NULL_COALESCE_036` | Generated deterministic SQLite parity case for SCALAR_NULL_COALESCE_036. |
| 0372 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_ARITH_037` | Generated deterministic SQLite parity case for SCALAR_ARITH_037. |
| 0373 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_CAST_TYPEOF_037` | Generated deterministic SQLite parity case for SCALAR_CAST_TYPEOF_037. |
| 0374 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_STRING_037` | Generated deterministic SQLite parity case for SCALAR_STRING_037. |
| 0375 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_NULL_COALESCE_037` | Generated deterministic SQLite parity case for SCALAR_NULL_COALESCE_037. |
| 0376 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_ARITH_038` | Generated deterministic SQLite parity case for SCALAR_ARITH_038. |
| 0377 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_CAST_TYPEOF_038` | Generated deterministic SQLite parity case for SCALAR_CAST_TYPEOF_038. |
| 0378 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_STRING_038` | Generated deterministic SQLite parity case for SCALAR_STRING_038. |
| 0379 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_NULL_COALESCE_038` | Generated deterministic SQLite parity case for SCALAR_NULL_COALESCE_038. |
| 0380 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_ARITH_039` | Generated deterministic SQLite parity case for SCALAR_ARITH_039. |
| 0381 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_CAST_TYPEOF_039` | Generated deterministic SQLite parity case for SCALAR_CAST_TYPEOF_039. |
| 0382 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_STRING_039` | Generated deterministic SQLite parity case for SCALAR_STRING_039. |
| 0383 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_NULL_COALESCE_039` | Generated deterministic SQLite parity case for SCALAR_NULL_COALESCE_039. |
| 0384 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_ARITH_040` | Generated deterministic SQLite parity case for SCALAR_ARITH_040. |
| 0385 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_CAST_TYPEOF_040` | Generated deterministic SQLite parity case for SCALAR_CAST_TYPEOF_040. |
| 0386 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_STRING_040` | Generated deterministic SQLite parity case for SCALAR_STRING_040. |
| 0387 | P1 | memory | GEN_SQL_SCALAR | `SCALAR_NULL_COALESCE_040` | Generated deterministic SQLite parity case for SCALAR_NULL_COALESCE_040. |
| 0388 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_001` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_001. |
| 0389 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_002` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_002. |
| 0390 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_003` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_003. |
| 0391 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_004` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_004. |
| 0392 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_005` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_005. |
| 0393 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_006` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_006. |
| 0394 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_007` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_007. |
| 0395 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_008` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_008. |
| 0396 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_009` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_009. |
| 0397 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_010` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_010. |
| 0398 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_011` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_011. |
| 0399 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_012` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_012. |
| 0400 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_013` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_013. |
| 0401 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_014` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_014. |
| 0402 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_015` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_015. |
| 0403 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_016` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_016. |
| 0404 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_017` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_017. |
| 0405 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_018` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_018. |
| 0406 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_019` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_019. |
| 0407 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_020` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_020. |
| 0408 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_021` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_021. |
| 0409 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_022` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_022. |
| 0410 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_023` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_023. |
| 0411 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_024` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_024. |
| 0412 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_025` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_025. |
| 0413 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_026` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_026. |
| 0414 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_027` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_027. |
| 0415 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_028` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_028. |
| 0416 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_029` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_029. |
| 0417 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_030` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_030. |
| 0418 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_031` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_031. |
| 0419 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_032` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_032. |
| 0420 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_033` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_033. |
| 0421 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_034` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_034. |
| 0422 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_035` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_035. |
| 0423 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_036` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_036. |
| 0424 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_037` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_037. |
| 0425 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_038` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_038. |
| 0426 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_039` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_039. |
| 0427 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_040` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_040. |
| 0428 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_041` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_041. |
| 0429 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_042` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_042. |
| 0430 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_043` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_043. |
| 0431 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_044` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_044. |
| 0432 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_045` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_045. |
| 0433 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_046` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_046. |
| 0434 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_047` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_047. |
| 0435 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_048` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_048. |
| 0436 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_049` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_049. |
| 0437 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_050` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_050. |
| 0438 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_051` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_051. |
| 0439 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_052` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_052. |
| 0440 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_053` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_053. |
| 0441 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_054` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_054. |
| 0442 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_055` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_055. |
| 0443 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_056` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_056. |
| 0444 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_057` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_057. |
| 0445 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_058` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_058. |
| 0446 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_059` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_059. |
| 0447 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_060` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_060. |
| 0448 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_061` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_061. |
| 0449 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_062` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_062. |
| 0450 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_063` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_063. |
| 0451 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_064` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_064. |
| 0452 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_065` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_065. |
| 0453 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_066` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_066. |
| 0454 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_067` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_067. |
| 0455 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_068` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_068. |
| 0456 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_069` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_069. |
| 0457 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_070` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_070. |
| 0458 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_071` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_071. |
| 0459 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_072` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_072. |
| 0460 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_073` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_073. |
| 0461 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_074` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_074. |
| 0462 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_075` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_075. |
| 0463 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_076` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_076. |
| 0464 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_077` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_077. |
| 0465 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_078` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_078. |
| 0466 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_079` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_079. |
| 0467 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_080` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_080. |
| 0468 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_081` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_081. |
| 0469 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_082` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_082. |
| 0470 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_083` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_083. |
| 0471 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_084` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_084. |
| 0472 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_085` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_085. |
| 0473 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_086` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_086. |
| 0474 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_087` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_087. |
| 0475 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_088` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_088. |
| 0476 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_089` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_089. |
| 0477 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_090` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_090. |
| 0478 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_091` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_091. |
| 0479 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_092` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_092. |
| 0480 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_093` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_093. |
| 0481 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_094` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_094. |
| 0482 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_095` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_095. |
| 0483 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_096` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_096. |
| 0484 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_097` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_097. |
| 0485 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_098` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_098. |
| 0486 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_099` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_099. |
| 0487 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_100` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_100. |
| 0488 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_101` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_101. |
| 0489 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_102` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_102. |
| 0490 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_103` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_103. |
| 0491 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_104` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_104. |
| 0492 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_105` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_105. |
| 0493 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_106` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_106. |
| 0494 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_107` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_107. |
| 0495 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_108` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_108. |
| 0496 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_109` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_109. |
| 0497 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_110` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_110. |
| 0498 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_111` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_111. |
| 0499 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_112` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_112. |
| 0500 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_113` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_113. |
| 0501 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_114` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_114. |
| 0502 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_115` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_115. |
| 0503 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_116` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_116. |
| 0504 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_117` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_117. |
| 0505 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_118` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_118. |
| 0506 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_119` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_119. |
| 0507 | P1 | memory | GEN_SQL_DML | `DML_WHERE_ORDER_LIMIT_120` | Generated deterministic SQLite parity case for DML_WHERE_ORDER_LIMIT_120. |
| 0508 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_001` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_001. |
| 0509 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_002` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_002. |
| 0510 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_003` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_003. |
| 0511 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_004` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_004. |
| 0512 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_005` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_005. |
| 0513 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_006` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_006. |
| 0514 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_007` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_007. |
| 0515 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_008` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_008. |
| 0516 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_009` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_009. |
| 0517 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_010` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_010. |
| 0518 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_011` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_011. |
| 0519 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_012` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_012. |
| 0520 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_013` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_013. |
| 0521 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_014` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_014. |
| 0522 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_015` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_015. |
| 0523 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_016` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_016. |
| 0524 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_017` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_017. |
| 0525 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_018` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_018. |
| 0526 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_019` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_019. |
| 0527 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_020` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_020. |
| 0528 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_021` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_021. |
| 0529 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_022` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_022. |
| 0530 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_023` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_023. |
| 0531 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_024` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_024. |
| 0532 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_025` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_025. |
| 0533 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_026` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_026. |
| 0534 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_027` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_027. |
| 0535 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_028` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_028. |
| 0536 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_029` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_029. |
| 0537 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_030` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_030. |
| 0538 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_031` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_031. |
| 0539 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_032` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_032. |
| 0540 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_033` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_033. |
| 0541 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_034` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_034. |
| 0542 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_035` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_035. |
| 0543 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_036` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_036. |
| 0544 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_037` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_037. |
| 0545 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_038` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_038. |
| 0546 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_039` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_039. |
| 0547 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_040` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_040. |
| 0548 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_041` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_041. |
| 0549 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_042` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_042. |
| 0550 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_043` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_043. |
| 0551 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_044` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_044. |
| 0552 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_045` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_045. |
| 0553 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_046` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_046. |
| 0554 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_047` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_047. |
| 0555 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_048` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_048. |
| 0556 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_049` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_049. |
| 0557 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_050` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_050. |
| 0558 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_051` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_051. |
| 0559 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_052` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_052. |
| 0560 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_053` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_053. |
| 0561 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_054` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_054. |
| 0562 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_055` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_055. |
| 0563 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_056` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_056. |
| 0564 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_057` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_057. |
| 0565 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_058` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_058. |
| 0566 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_059` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_059. |
| 0567 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_060` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_060. |
| 0568 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_061` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_061. |
| 0569 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_062` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_062. |
| 0570 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_063` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_063. |
| 0571 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_064` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_064. |
| 0572 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_065` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_065. |
| 0573 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_066` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_066. |
| 0574 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_067` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_067. |
| 0575 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_068` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_068. |
| 0576 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_069` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_069. |
| 0577 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_070` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_070. |
| 0578 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_071` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_071. |
| 0579 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_072` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_072. |
| 0580 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_073` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_073. |
| 0581 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_074` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_074. |
| 0582 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_075` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_075. |
| 0583 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_076` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_076. |
| 0584 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_077` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_077. |
| 0585 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_078` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_078. |
| 0586 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_079` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_079. |
| 0587 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_080` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_080. |
| 0588 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_081` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_081. |
| 0589 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_082` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_082. |
| 0590 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_083` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_083. |
| 0591 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_084` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_084. |
| 0592 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_085` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_085. |
| 0593 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_086` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_086. |
| 0594 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_087` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_087. |
| 0595 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_088` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_088. |
| 0596 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_089` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_089. |
| 0597 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_090` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_090. |
| 0598 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_091` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_091. |
| 0599 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_092` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_092. |
| 0600 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_093` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_093. |
| 0601 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_094` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_094. |
| 0602 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_095` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_095. |
| 0603 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_096` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_096. |
| 0604 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_097` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_097. |
| 0605 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_098` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_098. |
| 0606 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_099` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_099. |
| 0607 | P1 | memory | GEN_SQL_AGGREGATE | `AGG_GROUP_HAVING_100` | Generated deterministic SQLite parity case for AGG_GROUP_HAVING_100. |
| 0608 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_001` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_001. |
| 0609 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_002` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_002. |
| 0610 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_003` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_003. |
| 0611 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_004` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_004. |
| 0612 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_005` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_005. |
| 0613 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_006` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_006. |
| 0614 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_007` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_007. |
| 0615 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_008` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_008. |
| 0616 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_009` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_009. |
| 0617 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_010` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_010. |
| 0618 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_011` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_011. |
| 0619 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_012` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_012. |
| 0620 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_013` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_013. |
| 0621 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_014` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_014. |
| 0622 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_015` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_015. |
| 0623 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_016` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_016. |
| 0624 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_017` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_017. |
| 0625 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_018` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_018. |
| 0626 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_019` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_019. |
| 0627 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_020` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_020. |
| 0628 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_021` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_021. |
| 0629 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_022` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_022. |
| 0630 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_023` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_023. |
| 0631 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_024` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_024. |
| 0632 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_025` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_025. |
| 0633 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_026` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_026. |
| 0634 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_027` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_027. |
| 0635 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_028` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_028. |
| 0636 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_029` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_029. |
| 0637 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_030` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_030. |
| 0638 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_031` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_031. |
| 0639 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_032` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_032. |
| 0640 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_033` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_033. |
| 0641 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_034` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_034. |
| 0642 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_035` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_035. |
| 0643 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_036` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_036. |
| 0644 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_037` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_037. |
| 0645 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_038` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_038. |
| 0646 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_039` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_039. |
| 0647 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_040` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_040. |
| 0648 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_041` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_041. |
| 0649 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_042` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_042. |
| 0650 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_043` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_043. |
| 0651 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_044` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_044. |
| 0652 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_045` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_045. |
| 0653 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_046` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_046. |
| 0654 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_047` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_047. |
| 0655 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_048` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_048. |
| 0656 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_049` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_049. |
| 0657 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_050` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_050. |
| 0658 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_051` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_051. |
| 0659 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_052` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_052. |
| 0660 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_053` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_053. |
| 0661 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_054` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_054. |
| 0662 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_055` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_055. |
| 0663 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_056` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_056. |
| 0664 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_057` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_057. |
| 0665 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_058` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_058. |
| 0666 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_059` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_059. |
| 0667 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_060` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_060. |
| 0668 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_061` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_061. |
| 0669 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_062` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_062. |
| 0670 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_063` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_063. |
| 0671 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_064` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_064. |
| 0672 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_065` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_065. |
| 0673 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_066` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_066. |
| 0674 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_067` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_067. |
| 0675 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_068` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_068. |
| 0676 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_069` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_069. |
| 0677 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_070` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_070. |
| 0678 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_071` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_071. |
| 0679 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_072` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_072. |
| 0680 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_073` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_073. |
| 0681 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_074` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_074. |
| 0682 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_075` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_075. |
| 0683 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_076` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_076. |
| 0684 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_077` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_077. |
| 0685 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_078` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_078. |
| 0686 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_079` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_079. |
| 0687 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_080` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_080. |
| 0688 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_081` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_081. |
| 0689 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_082` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_082. |
| 0690 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_083` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_083. |
| 0691 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_084` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_084. |
| 0692 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_085` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_085. |
| 0693 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_086` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_086. |
| 0694 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_087` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_087. |
| 0695 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_088` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_088. |
| 0696 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_089` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_089. |
| 0697 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_090` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_090. |
| 0698 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_091` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_091. |
| 0699 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_092` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_092. |
| 0700 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_093` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_093. |
| 0701 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_094` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_094. |
| 0702 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_095` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_095. |
| 0703 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_096` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_096. |
| 0704 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_097` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_097. |
| 0705 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_098` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_098. |
| 0706 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_099` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_099. |
| 0707 | P1 | memory | GEN_SQL_JOIN_SUBQUERY | `JOIN_SUBQUERY_EXISTS_100` | Generated deterministic SQLite parity case for JOIN_SUBQUERY_EXISTS_100. |
| 0708 | P1 | memory | GEN_SQL_CTE | `CTE_RECURSIVE_MATRIX_001` | Generated deterministic SQLite parity case for CTE_RECURSIVE_MATRIX_001. |
| 0709 | P1 | memory | GEN_SQL_CTE | `CTE_RECURSIVE_MATRIX_002` | Generated deterministic SQLite parity case for CTE_RECURSIVE_MATRIX_002. |
| 0710 | P1 | memory | GEN_SQL_CTE | `CTE_RECURSIVE_MATRIX_003` | Generated deterministic SQLite parity case for CTE_RECURSIVE_MATRIX_003. |
| 0711 | P1 | memory | GEN_SQL_CTE | `CTE_RECURSIVE_MATRIX_004` | Generated deterministic SQLite parity case for CTE_RECURSIVE_MATRIX_004. |
| 0712 | P1 | memory | GEN_SQL_CTE | `CTE_RECURSIVE_MATRIX_005` | Generated deterministic SQLite parity case for CTE_RECURSIVE_MATRIX_005. |
| 0713 | P1 | memory | GEN_SQL_CTE | `CTE_RECURSIVE_MATRIX_006` | Generated deterministic SQLite parity case for CTE_RECURSIVE_MATRIX_006. |
| 0714 | P1 | memory | GEN_SQL_CTE | `CTE_RECURSIVE_MATRIX_007` | Generated deterministic SQLite parity case for CTE_RECURSIVE_MATRIX_007. |
| 0715 | P1 | memory | GEN_SQL_CTE | `CTE_RECURSIVE_MATRIX_008` | Generated deterministic SQLite parity case for CTE_RECURSIVE_MATRIX_008. |
| 0716 | P1 | memory | GEN_SQL_CTE | `CTE_RECURSIVE_MATRIX_009` | Generated deterministic SQLite parity case for CTE_RECURSIVE_MATRIX_009. |
| 0717 | P1 | memory | GEN_SQL_CTE | `CTE_RECURSIVE_MATRIX_010` | Generated deterministic SQLite parity case for CTE_RECURSIVE_MATRIX_010. |
| 0718 | P1 | memory | GEN_SQL_CTE | `CTE_RECURSIVE_MATRIX_011` | Generated deterministic SQLite parity case for CTE_RECURSIVE_MATRIX_011. |
| 0719 | P1 | memory | GEN_SQL_CTE | `CTE_RECURSIVE_MATRIX_012` | Generated deterministic SQLite parity case for CTE_RECURSIVE_MATRIX_012. |
| 0720 | P1 | memory | GEN_SQL_CTE | `CTE_RECURSIVE_MATRIX_013` | Generated deterministic SQLite parity case for CTE_RECURSIVE_MATRIX_013. |
| 0721 | P1 | memory | GEN_SQL_CTE | `CTE_RECURSIVE_MATRIX_014` | Generated deterministic SQLite parity case for CTE_RECURSIVE_MATRIX_014. |
| 0722 | P1 | memory | GEN_SQL_CTE | `CTE_RECURSIVE_MATRIX_015` | Generated deterministic SQLite parity case for CTE_RECURSIVE_MATRIX_015. |
| 0723 | P1 | memory | GEN_SQL_CTE | `CTE_RECURSIVE_MATRIX_016` | Generated deterministic SQLite parity case for CTE_RECURSIVE_MATRIX_016. |
| 0724 | P1 | memory | GEN_SQL_CTE | `CTE_RECURSIVE_MATRIX_017` | Generated deterministic SQLite parity case for CTE_RECURSIVE_MATRIX_017. |
| 0725 | P1 | memory | GEN_SQL_CTE | `CTE_RECURSIVE_MATRIX_018` | Generated deterministic SQLite parity case for CTE_RECURSIVE_MATRIX_018. |
| 0726 | P1 | memory | GEN_SQL_CTE | `CTE_RECURSIVE_MATRIX_019` | Generated deterministic SQLite parity case for CTE_RECURSIVE_MATRIX_019. |
| 0727 | P1 | memory | GEN_SQL_CTE | `CTE_RECURSIVE_MATRIX_020` | Generated deterministic SQLite parity case for CTE_RECURSIVE_MATRIX_020. |
| 0728 | P1 | memory | GEN_SQL_CTE | `CTE_RECURSIVE_MATRIX_021` | Generated deterministic SQLite parity case for CTE_RECURSIVE_MATRIX_021. |
| 0729 | P1 | memory | GEN_SQL_CTE | `CTE_RECURSIVE_MATRIX_022` | Generated deterministic SQLite parity case for CTE_RECURSIVE_MATRIX_022. |
| 0730 | P1 | memory | GEN_SQL_CTE | `CTE_RECURSIVE_MATRIX_023` | Generated deterministic SQLite parity case for CTE_RECURSIVE_MATRIX_023. |
| 0731 | P1 | memory | GEN_SQL_CTE | `CTE_RECURSIVE_MATRIX_024` | Generated deterministic SQLite parity case for CTE_RECURSIVE_MATRIX_024. |
| 0732 | P1 | memory | GEN_SQL_CTE | `CTE_RECURSIVE_MATRIX_025` | Generated deterministic SQLite parity case for CTE_RECURSIVE_MATRIX_025. |
| 0733 | P1 | memory | GEN_SQL_CTE | `CTE_RECURSIVE_MATRIX_026` | Generated deterministic SQLite parity case for CTE_RECURSIVE_MATRIX_026. |
| 0734 | P1 | memory | GEN_SQL_CTE | `CTE_RECURSIVE_MATRIX_027` | Generated deterministic SQLite parity case for CTE_RECURSIVE_MATRIX_027. |
| 0735 | P1 | memory | GEN_SQL_CTE | `CTE_RECURSIVE_MATRIX_028` | Generated deterministic SQLite parity case for CTE_RECURSIVE_MATRIX_028. |
| 0736 | P1 | memory | GEN_SQL_CTE | `CTE_RECURSIVE_MATRIX_029` | Generated deterministic SQLite parity case for CTE_RECURSIVE_MATRIX_029. |
| 0737 | P1 | memory | GEN_SQL_CTE | `CTE_RECURSIVE_MATRIX_030` | Generated deterministic SQLite parity case for CTE_RECURSIVE_MATRIX_030. |
| 0738 | P1 | memory | GEN_SQL_CTE | `CTE_RECURSIVE_MATRIX_031` | Generated deterministic SQLite parity case for CTE_RECURSIVE_MATRIX_031. |
| 0739 | P1 | memory | GEN_SQL_CTE | `CTE_RECURSIVE_MATRIX_032` | Generated deterministic SQLite parity case for CTE_RECURSIVE_MATRIX_032. |
| 0740 | P1 | memory | GEN_SQL_CTE | `CTE_RECURSIVE_MATRIX_033` | Generated deterministic SQLite parity case for CTE_RECURSIVE_MATRIX_033. |
| 0741 | P1 | memory | GEN_SQL_CTE | `CTE_RECURSIVE_MATRIX_034` | Generated deterministic SQLite parity case for CTE_RECURSIVE_MATRIX_034. |
| 0742 | P1 | memory | GEN_SQL_CTE | `CTE_RECURSIVE_MATRIX_035` | Generated deterministic SQLite parity case for CTE_RECURSIVE_MATRIX_035. |
| 0743 | P1 | memory | GEN_SQL_CTE | `CTE_RECURSIVE_MATRIX_036` | Generated deterministic SQLite parity case for CTE_RECURSIVE_MATRIX_036. |
| 0744 | P1 | memory | GEN_SQL_CTE | `CTE_RECURSIVE_MATRIX_037` | Generated deterministic SQLite parity case for CTE_RECURSIVE_MATRIX_037. |
| 0745 | P1 | memory | GEN_SQL_CTE | `CTE_RECURSIVE_MATRIX_038` | Generated deterministic SQLite parity case for CTE_RECURSIVE_MATRIX_038. |
| 0746 | P1 | memory | GEN_SQL_CTE | `CTE_RECURSIVE_MATRIX_039` | Generated deterministic SQLite parity case for CTE_RECURSIVE_MATRIX_039. |
| 0747 | P1 | memory | GEN_SQL_CTE | `CTE_RECURSIVE_MATRIX_040` | Generated deterministic SQLite parity case for CTE_RECURSIVE_MATRIX_040. |
| 0748 | P1 | memory | GEN_SQL_CTE | `CTE_RECURSIVE_MATRIX_041` | Generated deterministic SQLite parity case for CTE_RECURSIVE_MATRIX_041. |
| 0749 | P1 | memory | GEN_SQL_CTE | `CTE_RECURSIVE_MATRIX_042` | Generated deterministic SQLite parity case for CTE_RECURSIVE_MATRIX_042. |
| 0750 | P1 | memory | GEN_SQL_CTE | `CTE_RECURSIVE_MATRIX_043` | Generated deterministic SQLite parity case for CTE_RECURSIVE_MATRIX_043. |
| 0751 | P1 | memory | GEN_SQL_CTE | `CTE_RECURSIVE_MATRIX_044` | Generated deterministic SQLite parity case for CTE_RECURSIVE_MATRIX_044. |
| 0752 | P1 | memory | GEN_SQL_CTE | `CTE_RECURSIVE_MATRIX_045` | Generated deterministic SQLite parity case for CTE_RECURSIVE_MATRIX_045. |
| 0753 | P1 | memory | GEN_SQL_CTE | `CTE_RECURSIVE_MATRIX_046` | Generated deterministic SQLite parity case for CTE_RECURSIVE_MATRIX_046. |
| 0754 | P1 | memory | GEN_SQL_CTE | `CTE_RECURSIVE_MATRIX_047` | Generated deterministic SQLite parity case for CTE_RECURSIVE_MATRIX_047. |
| 0755 | P1 | memory | GEN_SQL_CTE | `CTE_RECURSIVE_MATRIX_048` | Generated deterministic SQLite parity case for CTE_RECURSIVE_MATRIX_048. |
| 0756 | P1 | memory | GEN_SQL_CTE | `CTE_RECURSIVE_MATRIX_049` | Generated deterministic SQLite parity case for CTE_RECURSIVE_MATRIX_049. |
| 0757 | P1 | memory | GEN_SQL_CTE | `CTE_RECURSIVE_MATRIX_050` | Generated deterministic SQLite parity case for CTE_RECURSIVE_MATRIX_050. |
| 0758 | P1 | memory | GEN_SQL_CTE | `CTE_RECURSIVE_MATRIX_051` | Generated deterministic SQLite parity case for CTE_RECURSIVE_MATRIX_051. |
| 0759 | P1 | memory | GEN_SQL_CTE | `CTE_RECURSIVE_MATRIX_052` | Generated deterministic SQLite parity case for CTE_RECURSIVE_MATRIX_052. |
| 0760 | P1 | memory | GEN_SQL_CTE | `CTE_RECURSIVE_MATRIX_053` | Generated deterministic SQLite parity case for CTE_RECURSIVE_MATRIX_053. |
| 0761 | P1 | memory | GEN_SQL_CTE | `CTE_RECURSIVE_MATRIX_054` | Generated deterministic SQLite parity case for CTE_RECURSIVE_MATRIX_054. |
| 0762 | P1 | memory | GEN_SQL_CTE | `CTE_RECURSIVE_MATRIX_055` | Generated deterministic SQLite parity case for CTE_RECURSIVE_MATRIX_055. |
| 0763 | P1 | memory | GEN_SQL_CTE | `CTE_RECURSIVE_MATRIX_056` | Generated deterministic SQLite parity case for CTE_RECURSIVE_MATRIX_056. |
| 0764 | P1 | memory | GEN_SQL_CTE | `CTE_RECURSIVE_MATRIX_057` | Generated deterministic SQLite parity case for CTE_RECURSIVE_MATRIX_057. |
| 0765 | P1 | memory | GEN_SQL_CTE | `CTE_RECURSIVE_MATRIX_058` | Generated deterministic SQLite parity case for CTE_RECURSIVE_MATRIX_058. |
| 0766 | P1 | memory | GEN_SQL_CTE | `CTE_RECURSIVE_MATRIX_059` | Generated deterministic SQLite parity case for CTE_RECURSIVE_MATRIX_059. |
| 0767 | P1 | memory | GEN_SQL_CTE | `CTE_RECURSIVE_MATRIX_060` | Generated deterministic SQLite parity case for CTE_RECURSIVE_MATRIX_060. |
| 0768 | P1 | memory | GEN_SQL_CTE | `CTE_RECURSIVE_MATRIX_061` | Generated deterministic SQLite parity case for CTE_RECURSIVE_MATRIX_061. |
| 0769 | P1 | memory | GEN_SQL_CTE | `CTE_RECURSIVE_MATRIX_062` | Generated deterministic SQLite parity case for CTE_RECURSIVE_MATRIX_062. |
| 0770 | P1 | memory | GEN_SQL_CTE | `CTE_RECURSIVE_MATRIX_063` | Generated deterministic SQLite parity case for CTE_RECURSIVE_MATRIX_063. |
| 0771 | P1 | memory | GEN_SQL_CTE | `CTE_RECURSIVE_MATRIX_064` | Generated deterministic SQLite parity case for CTE_RECURSIVE_MATRIX_064. |
| 0772 | P1 | memory | GEN_SQL_CTE | `CTE_RECURSIVE_MATRIX_065` | Generated deterministic SQLite parity case for CTE_RECURSIVE_MATRIX_065. |
| 0773 | P1 | memory | GEN_SQL_CTE | `CTE_RECURSIVE_MATRIX_066` | Generated deterministic SQLite parity case for CTE_RECURSIVE_MATRIX_066. |
| 0774 | P1 | memory | GEN_SQL_CTE | `CTE_RECURSIVE_MATRIX_067` | Generated deterministic SQLite parity case for CTE_RECURSIVE_MATRIX_067. |
| 0775 | P1 | memory | GEN_SQL_CTE | `CTE_RECURSIVE_MATRIX_068` | Generated deterministic SQLite parity case for CTE_RECURSIVE_MATRIX_068. |
| 0776 | P1 | memory | GEN_SQL_CTE | `CTE_RECURSIVE_MATRIX_069` | Generated deterministic SQLite parity case for CTE_RECURSIVE_MATRIX_069. |
| 0777 | P1 | memory | GEN_SQL_CTE | `CTE_RECURSIVE_MATRIX_070` | Generated deterministic SQLite parity case for CTE_RECURSIVE_MATRIX_070. |
| 0778 | P1 | memory | GEN_SQL_CTE | `CTE_RECURSIVE_MATRIX_071` | Generated deterministic SQLite parity case for CTE_RECURSIVE_MATRIX_071. |
| 0779 | P1 | memory | GEN_SQL_CTE | `CTE_RECURSIVE_MATRIX_072` | Generated deterministic SQLite parity case for CTE_RECURSIVE_MATRIX_072. |
| 0780 | P1 | memory | GEN_SQL_CTE | `CTE_RECURSIVE_MATRIX_073` | Generated deterministic SQLite parity case for CTE_RECURSIVE_MATRIX_073. |
| 0781 | P1 | memory | GEN_SQL_CTE | `CTE_RECURSIVE_MATRIX_074` | Generated deterministic SQLite parity case for CTE_RECURSIVE_MATRIX_074. |
| 0782 | P1 | memory | GEN_SQL_CTE | `CTE_RECURSIVE_MATRIX_075` | Generated deterministic SQLite parity case for CTE_RECURSIVE_MATRIX_075. |
| 0783 | P1 | memory | GEN_SQL_CTE | `CTE_RECURSIVE_MATRIX_076` | Generated deterministic SQLite parity case for CTE_RECURSIVE_MATRIX_076. |
| 0784 | P1 | memory | GEN_SQL_CTE | `CTE_RECURSIVE_MATRIX_077` | Generated deterministic SQLite parity case for CTE_RECURSIVE_MATRIX_077. |
| 0785 | P1 | memory | GEN_SQL_CTE | `CTE_RECURSIVE_MATRIX_078` | Generated deterministic SQLite parity case for CTE_RECURSIVE_MATRIX_078. |
| 0786 | P1 | memory | GEN_SQL_CTE | `CTE_RECURSIVE_MATRIX_079` | Generated deterministic SQLite parity case for CTE_RECURSIVE_MATRIX_079. |
| 0787 | P1 | memory | GEN_SQL_CTE | `CTE_RECURSIVE_MATRIX_080` | Generated deterministic SQLite parity case for CTE_RECURSIVE_MATRIX_080. |
| 0788 | P2 | memory | GEN_SQL_WINDOW | `WINDOW_PARTITION_SUM_001` | Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_001. |
| 0789 | P2 | memory | GEN_SQL_WINDOW | `WINDOW_PARTITION_SUM_002` | Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_002. |
| 0790 | P2 | memory | GEN_SQL_WINDOW | `WINDOW_PARTITION_SUM_003` | Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_003. |
| 0791 | P2 | memory | GEN_SQL_WINDOW | `WINDOW_PARTITION_SUM_004` | Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_004. |
| 0792 | P2 | memory | GEN_SQL_WINDOW | `WINDOW_PARTITION_SUM_005` | Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_005. |
| 0793 | P2 | memory | GEN_SQL_WINDOW | `WINDOW_PARTITION_SUM_006` | Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_006. |
| 0794 | P2 | memory | GEN_SQL_WINDOW | `WINDOW_PARTITION_SUM_007` | Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_007. |
| 0795 | P2 | memory | GEN_SQL_WINDOW | `WINDOW_PARTITION_SUM_008` | Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_008. |
| 0796 | P2 | memory | GEN_SQL_WINDOW | `WINDOW_PARTITION_SUM_009` | Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_009. |
| 0797 | P2 | memory | GEN_SQL_WINDOW | `WINDOW_PARTITION_SUM_010` | Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_010. |
| 0798 | P2 | memory | GEN_SQL_WINDOW | `WINDOW_PARTITION_SUM_011` | Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_011. |
| 0799 | P2 | memory | GEN_SQL_WINDOW | `WINDOW_PARTITION_SUM_012` | Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_012. |
| 0800 | P2 | memory | GEN_SQL_WINDOW | `WINDOW_PARTITION_SUM_013` | Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_013. |
| 0801 | P2 | memory | GEN_SQL_WINDOW | `WINDOW_PARTITION_SUM_014` | Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_014. |
| 0802 | P2 | memory | GEN_SQL_WINDOW | `WINDOW_PARTITION_SUM_015` | Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_015. |
| 0803 | P2 | memory | GEN_SQL_WINDOW | `WINDOW_PARTITION_SUM_016` | Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_016. |
| 0804 | P2 | memory | GEN_SQL_WINDOW | `WINDOW_PARTITION_SUM_017` | Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_017. |
| 0805 | P2 | memory | GEN_SQL_WINDOW | `WINDOW_PARTITION_SUM_018` | Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_018. |
| 0806 | P2 | memory | GEN_SQL_WINDOW | `WINDOW_PARTITION_SUM_019` | Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_019. |
| 0807 | P2 | memory | GEN_SQL_WINDOW | `WINDOW_PARTITION_SUM_020` | Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_020. |
| 0808 | P2 | memory | GEN_SQL_WINDOW | `WINDOW_PARTITION_SUM_021` | Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_021. |
| 0809 | P2 | memory | GEN_SQL_WINDOW | `WINDOW_PARTITION_SUM_022` | Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_022. |
| 0810 | P2 | memory | GEN_SQL_WINDOW | `WINDOW_PARTITION_SUM_023` | Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_023. |
| 0811 | P2 | memory | GEN_SQL_WINDOW | `WINDOW_PARTITION_SUM_024` | Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_024. |
| 0812 | P2 | memory | GEN_SQL_WINDOW | `WINDOW_PARTITION_SUM_025` | Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_025. |
| 0813 | P2 | memory | GEN_SQL_WINDOW | `WINDOW_PARTITION_SUM_026` | Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_026. |
| 0814 | P2 | memory | GEN_SQL_WINDOW | `WINDOW_PARTITION_SUM_027` | Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_027. |
| 0815 | P2 | memory | GEN_SQL_WINDOW | `WINDOW_PARTITION_SUM_028` | Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_028. |
| 0816 | P2 | memory | GEN_SQL_WINDOW | `WINDOW_PARTITION_SUM_029` | Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_029. |
| 0817 | P2 | memory | GEN_SQL_WINDOW | `WINDOW_PARTITION_SUM_030` | Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_030. |
| 0818 | P2 | memory | GEN_SQL_WINDOW | `WINDOW_PARTITION_SUM_031` | Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_031. |
| 0819 | P2 | memory | GEN_SQL_WINDOW | `WINDOW_PARTITION_SUM_032` | Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_032. |
| 0820 | P2 | memory | GEN_SQL_WINDOW | `WINDOW_PARTITION_SUM_033` | Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_033. |
| 0821 | P2 | memory | GEN_SQL_WINDOW | `WINDOW_PARTITION_SUM_034` | Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_034. |
| 0822 | P2 | memory | GEN_SQL_WINDOW | `WINDOW_PARTITION_SUM_035` | Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_035. |
| 0823 | P2 | memory | GEN_SQL_WINDOW | `WINDOW_PARTITION_SUM_036` | Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_036. |
| 0824 | P2 | memory | GEN_SQL_WINDOW | `WINDOW_PARTITION_SUM_037` | Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_037. |
| 0825 | P2 | memory | GEN_SQL_WINDOW | `WINDOW_PARTITION_SUM_038` | Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_038. |
| 0826 | P2 | memory | GEN_SQL_WINDOW | `WINDOW_PARTITION_SUM_039` | Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_039. |
| 0827 | P2 | memory | GEN_SQL_WINDOW | `WINDOW_PARTITION_SUM_040` | Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_040. |
| 0828 | P2 | memory | GEN_SQL_WINDOW | `WINDOW_PARTITION_SUM_041` | Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_041. |
| 0829 | P2 | memory | GEN_SQL_WINDOW | `WINDOW_PARTITION_SUM_042` | Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_042. |
| 0830 | P2 | memory | GEN_SQL_WINDOW | `WINDOW_PARTITION_SUM_043` | Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_043. |
| 0831 | P2 | memory | GEN_SQL_WINDOW | `WINDOW_PARTITION_SUM_044` | Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_044. |
| 0832 | P2 | memory | GEN_SQL_WINDOW | `WINDOW_PARTITION_SUM_045` | Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_045. |
| 0833 | P2 | memory | GEN_SQL_WINDOW | `WINDOW_PARTITION_SUM_046` | Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_046. |
| 0834 | P2 | memory | GEN_SQL_WINDOW | `WINDOW_PARTITION_SUM_047` | Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_047. |
| 0835 | P2 | memory | GEN_SQL_WINDOW | `WINDOW_PARTITION_SUM_048` | Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_048. |
| 0836 | P2 | memory | GEN_SQL_WINDOW | `WINDOW_PARTITION_SUM_049` | Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_049. |
| 0837 | P2 | memory | GEN_SQL_WINDOW | `WINDOW_PARTITION_SUM_050` | Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_050. |
| 0838 | P2 | memory | GEN_SQL_WINDOW | `WINDOW_PARTITION_SUM_051` | Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_051. |
| 0839 | P2 | memory | GEN_SQL_WINDOW | `WINDOW_PARTITION_SUM_052` | Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_052. |
| 0840 | P2 | memory | GEN_SQL_WINDOW | `WINDOW_PARTITION_SUM_053` | Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_053. |
| 0841 | P2 | memory | GEN_SQL_WINDOW | `WINDOW_PARTITION_SUM_054` | Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_054. |
| 0842 | P2 | memory | GEN_SQL_WINDOW | `WINDOW_PARTITION_SUM_055` | Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_055. |
| 0843 | P2 | memory | GEN_SQL_WINDOW | `WINDOW_PARTITION_SUM_056` | Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_056. |
| 0844 | P2 | memory | GEN_SQL_WINDOW | `WINDOW_PARTITION_SUM_057` | Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_057. |
| 0845 | P2 | memory | GEN_SQL_WINDOW | `WINDOW_PARTITION_SUM_058` | Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_058. |
| 0846 | P2 | memory | GEN_SQL_WINDOW | `WINDOW_PARTITION_SUM_059` | Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_059. |
| 0847 | P2 | memory | GEN_SQL_WINDOW | `WINDOW_PARTITION_SUM_060` | Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_060. |
| 0848 | P2 | memory | GEN_SQL_WINDOW | `WINDOW_PARTITION_SUM_061` | Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_061. |
| 0849 | P2 | memory | GEN_SQL_WINDOW | `WINDOW_PARTITION_SUM_062` | Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_062. |
| 0850 | P2 | memory | GEN_SQL_WINDOW | `WINDOW_PARTITION_SUM_063` | Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_063. |
| 0851 | P2 | memory | GEN_SQL_WINDOW | `WINDOW_PARTITION_SUM_064` | Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_064. |
| 0852 | P2 | memory | GEN_SQL_WINDOW | `WINDOW_PARTITION_SUM_065` | Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_065. |
| 0853 | P2 | memory | GEN_SQL_WINDOW | `WINDOW_PARTITION_SUM_066` | Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_066. |
| 0854 | P2 | memory | GEN_SQL_WINDOW | `WINDOW_PARTITION_SUM_067` | Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_067. |
| 0855 | P2 | memory | GEN_SQL_WINDOW | `WINDOW_PARTITION_SUM_068` | Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_068. |
| 0856 | P2 | memory | GEN_SQL_WINDOW | `WINDOW_PARTITION_SUM_069` | Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_069. |
| 0857 | P2 | memory | GEN_SQL_WINDOW | `WINDOW_PARTITION_SUM_070` | Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_070. |
| 0858 | P2 | memory | GEN_SQL_WINDOW | `WINDOW_PARTITION_SUM_071` | Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_071. |
| 0859 | P2 | memory | GEN_SQL_WINDOW | `WINDOW_PARTITION_SUM_072` | Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_072. |
| 0860 | P2 | memory | GEN_SQL_WINDOW | `WINDOW_PARTITION_SUM_073` | Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_073. |
| 0861 | P2 | memory | GEN_SQL_WINDOW | `WINDOW_PARTITION_SUM_074` | Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_074. |
| 0862 | P2 | memory | GEN_SQL_WINDOW | `WINDOW_PARTITION_SUM_075` | Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_075. |
| 0863 | P2 | memory | GEN_SQL_WINDOW | `WINDOW_PARTITION_SUM_076` | Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_076. |
| 0864 | P2 | memory | GEN_SQL_WINDOW | `WINDOW_PARTITION_SUM_077` | Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_077. |
| 0865 | P2 | memory | GEN_SQL_WINDOW | `WINDOW_PARTITION_SUM_078` | Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_078. |
| 0866 | P2 | memory | GEN_SQL_WINDOW | `WINDOW_PARTITION_SUM_079` | Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_079. |
| 0867 | P2 | memory | GEN_SQL_WINDOW | `WINDOW_PARTITION_SUM_080` | Generated deterministic SQLite parity case for WINDOW_PARTITION_SUM_080. |
| 0868 | P2 | memory | GEN_SQL_CONSTRAINT_TX | `CONSTRAINT_FK_SAVEPOINT_001` | Generated deterministic SQLite parity case for CONSTRAINT_FK_SAVEPOINT_001. |
| 0869 | P2 | memory | GEN_SQL_CONSTRAINT_TX | `CONSTRAINT_FK_SAVEPOINT_002` | Generated deterministic SQLite parity case for CONSTRAINT_FK_SAVEPOINT_002. |
| 0870 | P2 | memory | GEN_SQL_CONSTRAINT_TX | `CONSTRAINT_FK_SAVEPOINT_003` | Generated deterministic SQLite parity case for CONSTRAINT_FK_SAVEPOINT_003. |
| 0871 | P2 | memory | GEN_SQL_CONSTRAINT_TX | `CONSTRAINT_FK_SAVEPOINT_004` | Generated deterministic SQLite parity case for CONSTRAINT_FK_SAVEPOINT_004. |
| 0872 | P2 | memory | GEN_SQL_CONSTRAINT_TX | `CONSTRAINT_FK_SAVEPOINT_005` | Generated deterministic SQLite parity case for CONSTRAINT_FK_SAVEPOINT_005. |
| 0873 | P2 | memory | GEN_SQL_CONSTRAINT_TX | `CONSTRAINT_FK_SAVEPOINT_006` | Generated deterministic SQLite parity case for CONSTRAINT_FK_SAVEPOINT_006. |
| 0874 | P2 | memory | GEN_SQL_CONSTRAINT_TX | `CONSTRAINT_FK_SAVEPOINT_007` | Generated deterministic SQLite parity case for CONSTRAINT_FK_SAVEPOINT_007. |
| 0875 | P2 | memory | GEN_SQL_CONSTRAINT_TX | `CONSTRAINT_FK_SAVEPOINT_008` | Generated deterministic SQLite parity case for CONSTRAINT_FK_SAVEPOINT_008. |
| 0876 | P2 | memory | GEN_SQL_CONSTRAINT_TX | `CONSTRAINT_FK_SAVEPOINT_009` | Generated deterministic SQLite parity case for CONSTRAINT_FK_SAVEPOINT_009. |
| 0877 | P2 | memory | GEN_SQL_CONSTRAINT_TX | `CONSTRAINT_FK_SAVEPOINT_010` | Generated deterministic SQLite parity case for CONSTRAINT_FK_SAVEPOINT_010. |
| 0878 | P2 | memory | GEN_SQL_CONSTRAINT_TX | `CONSTRAINT_FK_SAVEPOINT_011` | Generated deterministic SQLite parity case for CONSTRAINT_FK_SAVEPOINT_011. |
| 0879 | P2 | memory | GEN_SQL_CONSTRAINT_TX | `CONSTRAINT_FK_SAVEPOINT_012` | Generated deterministic SQLite parity case for CONSTRAINT_FK_SAVEPOINT_012. |
| 0880 | P2 | memory | GEN_SQL_CONSTRAINT_TX | `CONSTRAINT_FK_SAVEPOINT_013` | Generated deterministic SQLite parity case for CONSTRAINT_FK_SAVEPOINT_013. |
| 0881 | P2 | memory | GEN_SQL_CONSTRAINT_TX | `CONSTRAINT_FK_SAVEPOINT_014` | Generated deterministic SQLite parity case for CONSTRAINT_FK_SAVEPOINT_014. |
| 0882 | P2 | memory | GEN_SQL_CONSTRAINT_TX | `CONSTRAINT_FK_SAVEPOINT_015` | Generated deterministic SQLite parity case for CONSTRAINT_FK_SAVEPOINT_015. |
| 0883 | P2 | memory | GEN_SQL_CONSTRAINT_TX | `CONSTRAINT_FK_SAVEPOINT_016` | Generated deterministic SQLite parity case for CONSTRAINT_FK_SAVEPOINT_016. |
| 0884 | P2 | memory | GEN_SQL_CONSTRAINT_TX | `CONSTRAINT_FK_SAVEPOINT_017` | Generated deterministic SQLite parity case for CONSTRAINT_FK_SAVEPOINT_017. |
| 0885 | P2 | memory | GEN_SQL_CONSTRAINT_TX | `CONSTRAINT_FK_SAVEPOINT_018` | Generated deterministic SQLite parity case for CONSTRAINT_FK_SAVEPOINT_018. |
| 0886 | P2 | memory | GEN_SQL_CONSTRAINT_TX | `CONSTRAINT_FK_SAVEPOINT_019` | Generated deterministic SQLite parity case for CONSTRAINT_FK_SAVEPOINT_019. |
| 0887 | P2 | memory | GEN_SQL_CONSTRAINT_TX | `CONSTRAINT_FK_SAVEPOINT_020` | Generated deterministic SQLite parity case for CONSTRAINT_FK_SAVEPOINT_020. |
| 0888 | P2 | memory | GEN_SQL_CONSTRAINT_TX | `CONSTRAINT_FK_SAVEPOINT_021` | Generated deterministic SQLite parity case for CONSTRAINT_FK_SAVEPOINT_021. |
| 0889 | P2 | memory | GEN_SQL_CONSTRAINT_TX | `CONSTRAINT_FK_SAVEPOINT_022` | Generated deterministic SQLite parity case for CONSTRAINT_FK_SAVEPOINT_022. |
| 0890 | P2 | memory | GEN_SQL_CONSTRAINT_TX | `CONSTRAINT_FK_SAVEPOINT_023` | Generated deterministic SQLite parity case for CONSTRAINT_FK_SAVEPOINT_023. |
| 0891 | P2 | memory | GEN_SQL_CONSTRAINT_TX | `CONSTRAINT_FK_SAVEPOINT_024` | Generated deterministic SQLite parity case for CONSTRAINT_FK_SAVEPOINT_024. |
| 0892 | P2 | memory | GEN_SQL_CONSTRAINT_TX | `CONSTRAINT_FK_SAVEPOINT_025` | Generated deterministic SQLite parity case for CONSTRAINT_FK_SAVEPOINT_025. |
| 0893 | P2 | memory | GEN_SQL_CONSTRAINT_TX | `CONSTRAINT_FK_SAVEPOINT_026` | Generated deterministic SQLite parity case for CONSTRAINT_FK_SAVEPOINT_026. |
| 0894 | P2 | memory | GEN_SQL_CONSTRAINT_TX | `CONSTRAINT_FK_SAVEPOINT_027` | Generated deterministic SQLite parity case for CONSTRAINT_FK_SAVEPOINT_027. |
| 0895 | P2 | memory | GEN_SQL_CONSTRAINT_TX | `CONSTRAINT_FK_SAVEPOINT_028` | Generated deterministic SQLite parity case for CONSTRAINT_FK_SAVEPOINT_028. |
| 0896 | P2 | memory | GEN_SQL_CONSTRAINT_TX | `CONSTRAINT_FK_SAVEPOINT_029` | Generated deterministic SQLite parity case for CONSTRAINT_FK_SAVEPOINT_029. |
| 0897 | P2 | memory | GEN_SQL_CONSTRAINT_TX | `CONSTRAINT_FK_SAVEPOINT_030` | Generated deterministic SQLite parity case for CONSTRAINT_FK_SAVEPOINT_030. |
| 0898 | P2 | memory | GEN_SQL_CONSTRAINT_TX | `CONSTRAINT_FK_SAVEPOINT_031` | Generated deterministic SQLite parity case for CONSTRAINT_FK_SAVEPOINT_031. |
| 0899 | P2 | memory | GEN_SQL_CONSTRAINT_TX | `CONSTRAINT_FK_SAVEPOINT_032` | Generated deterministic SQLite parity case for CONSTRAINT_FK_SAVEPOINT_032. |
| 0900 | P2 | memory | GEN_SQL_CONSTRAINT_TX | `CONSTRAINT_FK_SAVEPOINT_033` | Generated deterministic SQLite parity case for CONSTRAINT_FK_SAVEPOINT_033. |
| 0901 | P2 | memory | GEN_SQL_CONSTRAINT_TX | `CONSTRAINT_FK_SAVEPOINT_034` | Generated deterministic SQLite parity case for CONSTRAINT_FK_SAVEPOINT_034. |
| 0902 | P2 | memory | GEN_SQL_CONSTRAINT_TX | `CONSTRAINT_FK_SAVEPOINT_035` | Generated deterministic SQLite parity case for CONSTRAINT_FK_SAVEPOINT_035. |
| 0903 | P2 | memory | GEN_SQL_CONSTRAINT_TX | `CONSTRAINT_FK_SAVEPOINT_036` | Generated deterministic SQLite parity case for CONSTRAINT_FK_SAVEPOINT_036. |
| 0904 | P2 | memory | GEN_SQL_CONSTRAINT_TX | `CONSTRAINT_FK_SAVEPOINT_037` | Generated deterministic SQLite parity case for CONSTRAINT_FK_SAVEPOINT_037. |
| 0905 | P2 | memory | GEN_SQL_CONSTRAINT_TX | `CONSTRAINT_FK_SAVEPOINT_038` | Generated deterministic SQLite parity case for CONSTRAINT_FK_SAVEPOINT_038. |
| 0906 | P2 | memory | GEN_SQL_CONSTRAINT_TX | `CONSTRAINT_FK_SAVEPOINT_039` | Generated deterministic SQLite parity case for CONSTRAINT_FK_SAVEPOINT_039. |
| 0907 | P2 | memory | GEN_SQL_CONSTRAINT_TX | `CONSTRAINT_FK_SAVEPOINT_040` | Generated deterministic SQLite parity case for CONSTRAINT_FK_SAVEPOINT_040. |
| 0908 | P2 | memory | GEN_SQL_CONSTRAINT_TX | `CONSTRAINT_FK_SAVEPOINT_041` | Generated deterministic SQLite parity case for CONSTRAINT_FK_SAVEPOINT_041. |
| 0909 | P2 | memory | GEN_SQL_CONSTRAINT_TX | `CONSTRAINT_FK_SAVEPOINT_042` | Generated deterministic SQLite parity case for CONSTRAINT_FK_SAVEPOINT_042. |
| 0910 | P2 | memory | GEN_SQL_CONSTRAINT_TX | `CONSTRAINT_FK_SAVEPOINT_043` | Generated deterministic SQLite parity case for CONSTRAINT_FK_SAVEPOINT_043. |
| 0911 | P2 | memory | GEN_SQL_CONSTRAINT_TX | `CONSTRAINT_FK_SAVEPOINT_044` | Generated deterministic SQLite parity case for CONSTRAINT_FK_SAVEPOINT_044. |
| 0912 | P2 | memory | GEN_SQL_CONSTRAINT_TX | `CONSTRAINT_FK_SAVEPOINT_045` | Generated deterministic SQLite parity case for CONSTRAINT_FK_SAVEPOINT_045. |
| 0913 | P2 | memory | GEN_SQL_CONSTRAINT_TX | `CONSTRAINT_FK_SAVEPOINT_046` | Generated deterministic SQLite parity case for CONSTRAINT_FK_SAVEPOINT_046. |
| 0914 | P2 | memory | GEN_SQL_CONSTRAINT_TX | `CONSTRAINT_FK_SAVEPOINT_047` | Generated deterministic SQLite parity case for CONSTRAINT_FK_SAVEPOINT_047. |
| 0915 | P2 | memory | GEN_SQL_CONSTRAINT_TX | `CONSTRAINT_FK_SAVEPOINT_048` | Generated deterministic SQLite parity case for CONSTRAINT_FK_SAVEPOINT_048. |
| 0916 | P2 | memory | GEN_SQL_CONSTRAINT_TX | `CONSTRAINT_FK_SAVEPOINT_049` | Generated deterministic SQLite parity case for CONSTRAINT_FK_SAVEPOINT_049. |
| 0917 | P2 | memory | GEN_SQL_CONSTRAINT_TX | `CONSTRAINT_FK_SAVEPOINT_050` | Generated deterministic SQLite parity case for CONSTRAINT_FK_SAVEPOINT_050. |
| 0918 | P2 | memory | GEN_SQL_CONSTRAINT_TX | `CONSTRAINT_FK_SAVEPOINT_051` | Generated deterministic SQLite parity case for CONSTRAINT_FK_SAVEPOINT_051. |
| 0919 | P2 | memory | GEN_SQL_CONSTRAINT_TX | `CONSTRAINT_FK_SAVEPOINT_052` | Generated deterministic SQLite parity case for CONSTRAINT_FK_SAVEPOINT_052. |
| 0920 | P2 | memory | GEN_SQL_CONSTRAINT_TX | `CONSTRAINT_FK_SAVEPOINT_053` | Generated deterministic SQLite parity case for CONSTRAINT_FK_SAVEPOINT_053. |
| 0921 | P2 | memory | GEN_SQL_CONSTRAINT_TX | `CONSTRAINT_FK_SAVEPOINT_054` | Generated deterministic SQLite parity case for CONSTRAINT_FK_SAVEPOINT_054. |
| 0922 | P2 | memory | GEN_SQL_CONSTRAINT_TX | `CONSTRAINT_FK_SAVEPOINT_055` | Generated deterministic SQLite parity case for CONSTRAINT_FK_SAVEPOINT_055. |
| 0923 | P2 | memory | GEN_SQL_CONSTRAINT_TX | `CONSTRAINT_FK_SAVEPOINT_056` | Generated deterministic SQLite parity case for CONSTRAINT_FK_SAVEPOINT_056. |
| 0924 | P2 | memory | GEN_SQL_CONSTRAINT_TX | `CONSTRAINT_FK_SAVEPOINT_057` | Generated deterministic SQLite parity case for CONSTRAINT_FK_SAVEPOINT_057. |
| 0925 | P2 | memory | GEN_SQL_CONSTRAINT_TX | `CONSTRAINT_FK_SAVEPOINT_058` | Generated deterministic SQLite parity case for CONSTRAINT_FK_SAVEPOINT_058. |
| 0926 | P2 | memory | GEN_SQL_CONSTRAINT_TX | `CONSTRAINT_FK_SAVEPOINT_059` | Generated deterministic SQLite parity case for CONSTRAINT_FK_SAVEPOINT_059. |
| 0927 | P2 | memory | GEN_SQL_CONSTRAINT_TX | `CONSTRAINT_FK_SAVEPOINT_060` | Generated deterministic SQLite parity case for CONSTRAINT_FK_SAVEPOINT_060. |
| 0928 | P2 | memory | GEN_SQL_CONSTRAINT_TX | `CONSTRAINT_FK_SAVEPOINT_061` | Generated deterministic SQLite parity case for CONSTRAINT_FK_SAVEPOINT_061. |
| 0929 | P2 | memory | GEN_SQL_CONSTRAINT_TX | `CONSTRAINT_FK_SAVEPOINT_062` | Generated deterministic SQLite parity case for CONSTRAINT_FK_SAVEPOINT_062. |
| 0930 | P2 | memory | GEN_SQL_CONSTRAINT_TX | `CONSTRAINT_FK_SAVEPOINT_063` | Generated deterministic SQLite parity case for CONSTRAINT_FK_SAVEPOINT_063. |
| 0931 | P2 | memory | GEN_SQL_CONSTRAINT_TX | `CONSTRAINT_FK_SAVEPOINT_064` | Generated deterministic SQLite parity case for CONSTRAINT_FK_SAVEPOINT_064. |
| 0932 | P2 | memory | GEN_SQL_CONSTRAINT_TX | `CONSTRAINT_FK_SAVEPOINT_065` | Generated deterministic SQLite parity case for CONSTRAINT_FK_SAVEPOINT_065. |
| 0933 | P2 | memory | GEN_SQL_CONSTRAINT_TX | `CONSTRAINT_FK_SAVEPOINT_066` | Generated deterministic SQLite parity case for CONSTRAINT_FK_SAVEPOINT_066. |
| 0934 | P2 | memory | GEN_SQL_CONSTRAINT_TX | `CONSTRAINT_FK_SAVEPOINT_067` | Generated deterministic SQLite parity case for CONSTRAINT_FK_SAVEPOINT_067. |
| 0935 | P2 | memory | GEN_SQL_CONSTRAINT_TX | `CONSTRAINT_FK_SAVEPOINT_068` | Generated deterministic SQLite parity case for CONSTRAINT_FK_SAVEPOINT_068. |
| 0936 | P2 | memory | GEN_SQL_CONSTRAINT_TX | `CONSTRAINT_FK_SAVEPOINT_069` | Generated deterministic SQLite parity case for CONSTRAINT_FK_SAVEPOINT_069. |
| 0937 | P2 | memory | GEN_SQL_CONSTRAINT_TX | `CONSTRAINT_FK_SAVEPOINT_070` | Generated deterministic SQLite parity case for CONSTRAINT_FK_SAVEPOINT_070. |
| 0938 | P2 | memory | GEN_SQL_CONSTRAINT_TX | `CONSTRAINT_FK_SAVEPOINT_071` | Generated deterministic SQLite parity case for CONSTRAINT_FK_SAVEPOINT_071. |
| 0939 | P2 | memory | GEN_SQL_CONSTRAINT_TX | `CONSTRAINT_FK_SAVEPOINT_072` | Generated deterministic SQLite parity case for CONSTRAINT_FK_SAVEPOINT_072. |
| 0940 | P2 | memory | GEN_SQL_CONSTRAINT_TX | `CONSTRAINT_FK_SAVEPOINT_073` | Generated deterministic SQLite parity case for CONSTRAINT_FK_SAVEPOINT_073. |
| 0941 | P2 | memory | GEN_SQL_CONSTRAINT_TX | `CONSTRAINT_FK_SAVEPOINT_074` | Generated deterministic SQLite parity case for CONSTRAINT_FK_SAVEPOINT_074. |
| 0942 | P2 | memory | GEN_SQL_CONSTRAINT_TX | `CONSTRAINT_FK_SAVEPOINT_075` | Generated deterministic SQLite parity case for CONSTRAINT_FK_SAVEPOINT_075. |
| 0943 | P2 | memory | GEN_SQL_CONSTRAINT_TX | `CONSTRAINT_FK_SAVEPOINT_076` | Generated deterministic SQLite parity case for CONSTRAINT_FK_SAVEPOINT_076. |
| 0944 | P2 | memory | GEN_SQL_CONSTRAINT_TX | `CONSTRAINT_FK_SAVEPOINT_077` | Generated deterministic SQLite parity case for CONSTRAINT_FK_SAVEPOINT_077. |
| 0945 | P2 | memory | GEN_SQL_CONSTRAINT_TX | `CONSTRAINT_FK_SAVEPOINT_078` | Generated deterministic SQLite parity case for CONSTRAINT_FK_SAVEPOINT_078. |
| 0946 | P2 | memory | GEN_SQL_CONSTRAINT_TX | `CONSTRAINT_FK_SAVEPOINT_079` | Generated deterministic SQLite parity case for CONSTRAINT_FK_SAVEPOINT_079. |
| 0947 | P2 | memory | GEN_SQL_CONSTRAINT_TX | `CONSTRAINT_FK_SAVEPOINT_080` | Generated deterministic SQLite parity case for CONSTRAINT_FK_SAVEPOINT_080. |
| 0948 | P2 | memory | GEN_SQL_VIEW_TRIGGER | `VIEW_TRIGGER_GENERATED_001` | Generated deterministic SQLite parity case for VIEW_TRIGGER_GENERATED_001. |
| 0949 | P2 | memory | GEN_SQL_VIEW_TRIGGER | `VIEW_TRIGGER_GENERATED_002` | Generated deterministic SQLite parity case for VIEW_TRIGGER_GENERATED_002. |
| 0950 | P2 | memory | GEN_SQL_VIEW_TRIGGER | `VIEW_TRIGGER_GENERATED_003` | Generated deterministic SQLite parity case for VIEW_TRIGGER_GENERATED_003. |
| 0951 | P2 | memory | GEN_SQL_VIEW_TRIGGER | `VIEW_TRIGGER_GENERATED_004` | Generated deterministic SQLite parity case for VIEW_TRIGGER_GENERATED_004. |
| 0952 | P2 | memory | GEN_SQL_VIEW_TRIGGER | `VIEW_TRIGGER_GENERATED_005` | Generated deterministic SQLite parity case for VIEW_TRIGGER_GENERATED_005. |
| 0953 | P2 | memory | GEN_SQL_VIEW_TRIGGER | `VIEW_TRIGGER_GENERATED_006` | Generated deterministic SQLite parity case for VIEW_TRIGGER_GENERATED_006. |
| 0954 | P2 | memory | GEN_SQL_VIEW_TRIGGER | `VIEW_TRIGGER_GENERATED_007` | Generated deterministic SQLite parity case for VIEW_TRIGGER_GENERATED_007. |
| 0955 | P2 | memory | GEN_SQL_VIEW_TRIGGER | `VIEW_TRIGGER_GENERATED_008` | Generated deterministic SQLite parity case for VIEW_TRIGGER_GENERATED_008. |
| 0956 | P2 | memory | GEN_SQL_VIEW_TRIGGER | `VIEW_TRIGGER_GENERATED_009` | Generated deterministic SQLite parity case for VIEW_TRIGGER_GENERATED_009. |
| 0957 | P2 | memory | GEN_SQL_VIEW_TRIGGER | `VIEW_TRIGGER_GENERATED_010` | Generated deterministic SQLite parity case for VIEW_TRIGGER_GENERATED_010. |
| 0958 | P2 | memory | GEN_SQL_VIEW_TRIGGER | `VIEW_TRIGGER_GENERATED_011` | Generated deterministic SQLite parity case for VIEW_TRIGGER_GENERATED_011. |
| 0959 | P2 | memory | GEN_SQL_VIEW_TRIGGER | `VIEW_TRIGGER_GENERATED_012` | Generated deterministic SQLite parity case for VIEW_TRIGGER_GENERATED_012. |
| 0960 | P2 | memory | GEN_SQL_VIEW_TRIGGER | `VIEW_TRIGGER_GENERATED_013` | Generated deterministic SQLite parity case for VIEW_TRIGGER_GENERATED_013. |
| 0961 | P2 | memory | GEN_SQL_VIEW_TRIGGER | `VIEW_TRIGGER_GENERATED_014` | Generated deterministic SQLite parity case for VIEW_TRIGGER_GENERATED_014. |
| 0962 | P2 | memory | GEN_SQL_VIEW_TRIGGER | `VIEW_TRIGGER_GENERATED_015` | Generated deterministic SQLite parity case for VIEW_TRIGGER_GENERATED_015. |
| 0963 | P2 | memory | GEN_SQL_VIEW_TRIGGER | `VIEW_TRIGGER_GENERATED_016` | Generated deterministic SQLite parity case for VIEW_TRIGGER_GENERATED_016. |
| 0964 | P2 | memory | GEN_SQL_VIEW_TRIGGER | `VIEW_TRIGGER_GENERATED_017` | Generated deterministic SQLite parity case for VIEW_TRIGGER_GENERATED_017. |
| 0965 | P2 | memory | GEN_SQL_VIEW_TRIGGER | `VIEW_TRIGGER_GENERATED_018` | Generated deterministic SQLite parity case for VIEW_TRIGGER_GENERATED_018. |
| 0966 | P2 | memory | GEN_SQL_VIEW_TRIGGER | `VIEW_TRIGGER_GENERATED_019` | Generated deterministic SQLite parity case for VIEW_TRIGGER_GENERATED_019. |
| 0967 | P2 | memory | GEN_SQL_VIEW_TRIGGER | `VIEW_TRIGGER_GENERATED_020` | Generated deterministic SQLite parity case for VIEW_TRIGGER_GENERATED_020. |
| 0968 | P2 | memory | GEN_SQL_VIEW_TRIGGER | `VIEW_TRIGGER_GENERATED_021` | Generated deterministic SQLite parity case for VIEW_TRIGGER_GENERATED_021. |
| 0969 | P2 | memory | GEN_SQL_VIEW_TRIGGER | `VIEW_TRIGGER_GENERATED_022` | Generated deterministic SQLite parity case for VIEW_TRIGGER_GENERATED_022. |
| 0970 | P2 | memory | GEN_SQL_VIEW_TRIGGER | `VIEW_TRIGGER_GENERATED_023` | Generated deterministic SQLite parity case for VIEW_TRIGGER_GENERATED_023. |
| 0971 | P2 | memory | GEN_SQL_VIEW_TRIGGER | `VIEW_TRIGGER_GENERATED_024` | Generated deterministic SQLite parity case for VIEW_TRIGGER_GENERATED_024. |
| 0972 | P2 | memory | GEN_SQL_VIEW_TRIGGER | `VIEW_TRIGGER_GENERATED_025` | Generated deterministic SQLite parity case for VIEW_TRIGGER_GENERATED_025. |
| 0973 | P2 | memory | GEN_SQL_VIEW_TRIGGER | `VIEW_TRIGGER_GENERATED_026` | Generated deterministic SQLite parity case for VIEW_TRIGGER_GENERATED_026. |
| 0974 | P2 | memory | GEN_SQL_VIEW_TRIGGER | `VIEW_TRIGGER_GENERATED_027` | Generated deterministic SQLite parity case for VIEW_TRIGGER_GENERATED_027. |
| 0975 | P2 | memory | GEN_SQL_VIEW_TRIGGER | `VIEW_TRIGGER_GENERATED_028` | Generated deterministic SQLite parity case for VIEW_TRIGGER_GENERATED_028. |
| 0976 | P2 | memory | GEN_SQL_VIEW_TRIGGER | `VIEW_TRIGGER_GENERATED_029` | Generated deterministic SQLite parity case for VIEW_TRIGGER_GENERATED_029. |
| 0977 | P2 | memory | GEN_SQL_VIEW_TRIGGER | `VIEW_TRIGGER_GENERATED_030` | Generated deterministic SQLite parity case for VIEW_TRIGGER_GENERATED_030. |
| 0978 | P2 | memory | GEN_SQL_VIEW_TRIGGER | `VIEW_TRIGGER_GENERATED_031` | Generated deterministic SQLite parity case for VIEW_TRIGGER_GENERATED_031. |
| 0979 | P2 | memory | GEN_SQL_VIEW_TRIGGER | `VIEW_TRIGGER_GENERATED_032` | Generated deterministic SQLite parity case for VIEW_TRIGGER_GENERATED_032. |
| 0980 | P2 | memory | GEN_SQL_VIEW_TRIGGER | `VIEW_TRIGGER_GENERATED_033` | Generated deterministic SQLite parity case for VIEW_TRIGGER_GENERATED_033. |
| 0981 | P2 | memory | GEN_SQL_VIEW_TRIGGER | `VIEW_TRIGGER_GENERATED_034` | Generated deterministic SQLite parity case for VIEW_TRIGGER_GENERATED_034. |
| 0982 | P2 | memory | GEN_SQL_VIEW_TRIGGER | `VIEW_TRIGGER_GENERATED_035` | Generated deterministic SQLite parity case for VIEW_TRIGGER_GENERATED_035. |
| 0983 | P2 | memory | GEN_SQL_VIEW_TRIGGER | `VIEW_TRIGGER_GENERATED_036` | Generated deterministic SQLite parity case for VIEW_TRIGGER_GENERATED_036. |
| 0984 | P2 | memory | GEN_SQL_VIEW_TRIGGER | `VIEW_TRIGGER_GENERATED_037` | Generated deterministic SQLite parity case for VIEW_TRIGGER_GENERATED_037. |
| 0985 | P2 | memory | GEN_SQL_VIEW_TRIGGER | `VIEW_TRIGGER_GENERATED_038` | Generated deterministic SQLite parity case for VIEW_TRIGGER_GENERATED_038. |
| 0986 | P2 | memory | GEN_SQL_VIEW_TRIGGER | `VIEW_TRIGGER_GENERATED_039` | Generated deterministic SQLite parity case for VIEW_TRIGGER_GENERATED_039. |
| 0987 | P2 | memory | GEN_SQL_VIEW_TRIGGER | `VIEW_TRIGGER_GENERATED_040` | Generated deterministic SQLite parity case for VIEW_TRIGGER_GENERATED_040. |
| 0988 | P2 | memory | GEN_SQL_VIEW_TRIGGER | `VIEW_TRIGGER_GENERATED_041` | Generated deterministic SQLite parity case for VIEW_TRIGGER_GENERATED_041. |
| 0989 | P2 | memory | GEN_SQL_VIEW_TRIGGER | `VIEW_TRIGGER_GENERATED_042` | Generated deterministic SQLite parity case for VIEW_TRIGGER_GENERATED_042. |
| 0990 | P2 | memory | GEN_SQL_VIEW_TRIGGER | `VIEW_TRIGGER_GENERATED_043` | Generated deterministic SQLite parity case for VIEW_TRIGGER_GENERATED_043. |
| 0991 | P2 | memory | GEN_SQL_VIEW_TRIGGER | `VIEW_TRIGGER_GENERATED_044` | Generated deterministic SQLite parity case for VIEW_TRIGGER_GENERATED_044. |
| 0992 | P2 | memory | GEN_SQL_VIEW_TRIGGER | `VIEW_TRIGGER_GENERATED_045` | Generated deterministic SQLite parity case for VIEW_TRIGGER_GENERATED_045. |
| 0993 | P2 | memory | GEN_SQL_VIEW_TRIGGER | `VIEW_TRIGGER_GENERATED_046` | Generated deterministic SQLite parity case for VIEW_TRIGGER_GENERATED_046. |
| 0994 | P2 | memory | GEN_SQL_VIEW_TRIGGER | `VIEW_TRIGGER_GENERATED_047` | Generated deterministic SQLite parity case for VIEW_TRIGGER_GENERATED_047. |
| 0995 | P2 | memory | GEN_SQL_VIEW_TRIGGER | `VIEW_TRIGGER_GENERATED_048` | Generated deterministic SQLite parity case for VIEW_TRIGGER_GENERATED_048. |
| 0996 | P2 | memory | GEN_SQL_VIEW_TRIGGER | `VIEW_TRIGGER_GENERATED_049` | Generated deterministic SQLite parity case for VIEW_TRIGGER_GENERATED_049. |
| 0997 | P2 | memory | GEN_SQL_VIEW_TRIGGER | `VIEW_TRIGGER_GENERATED_050` | Generated deterministic SQLite parity case for VIEW_TRIGGER_GENERATED_050. |
| 0998 | P2 | memory | GEN_SQL_VIEW_TRIGGER | `VIEW_TRIGGER_GENERATED_051` | Generated deterministic SQLite parity case for VIEW_TRIGGER_GENERATED_051. |
| 0999 | P2 | memory | GEN_SQL_VIEW_TRIGGER | `VIEW_TRIGGER_GENERATED_052` | Generated deterministic SQLite parity case for VIEW_TRIGGER_GENERATED_052. |
| 1000 | P2 | memory | GEN_SQL_VIEW_TRIGGER | `VIEW_TRIGGER_GENERATED_053` | Generated deterministic SQLite parity case for VIEW_TRIGGER_GENERATED_053. |
| 1001 | P2 | memory | GEN_SQL_VIEW_TRIGGER | `VIEW_TRIGGER_GENERATED_054` | Generated deterministic SQLite parity case for VIEW_TRIGGER_GENERATED_054. |
| 1002 | P2 | memory | GEN_SQL_VIEW_TRIGGER | `VIEW_TRIGGER_GENERATED_055` | Generated deterministic SQLite parity case for VIEW_TRIGGER_GENERATED_055. |
| 1003 | P2 | memory | GEN_SQL_VIEW_TRIGGER | `VIEW_TRIGGER_GENERATED_056` | Generated deterministic SQLite parity case for VIEW_TRIGGER_GENERATED_056. |
| 1004 | P2 | memory | GEN_SQL_VIEW_TRIGGER | `VIEW_TRIGGER_GENERATED_057` | Generated deterministic SQLite parity case for VIEW_TRIGGER_GENERATED_057. |
| 1005 | P2 | memory | GEN_SQL_VIEW_TRIGGER | `VIEW_TRIGGER_GENERATED_058` | Generated deterministic SQLite parity case for VIEW_TRIGGER_GENERATED_058. |
| 1006 | P2 | memory | GEN_SQL_VIEW_TRIGGER | `VIEW_TRIGGER_GENERATED_059` | Generated deterministic SQLite parity case for VIEW_TRIGGER_GENERATED_059. |
| 1007 | P2 | memory | GEN_SQL_VIEW_TRIGGER | `VIEW_TRIGGER_GENERATED_060` | Generated deterministic SQLite parity case for VIEW_TRIGGER_GENERATED_060. |
| 1008 | P2 | memory | GEN_SQL_JSON | `JSON_EXTRACT_SET_001` | Generated deterministic SQLite parity case for JSON_EXTRACT_SET_001. |
| 1009 | P2 | memory | GEN_SQL_JSON | `JSON_EXTRACT_SET_002` | Generated deterministic SQLite parity case for JSON_EXTRACT_SET_002. |
| 1010 | P2 | memory | GEN_SQL_JSON | `JSON_EXTRACT_SET_003` | Generated deterministic SQLite parity case for JSON_EXTRACT_SET_003. |
| 1011 | P2 | memory | GEN_SQL_JSON | `JSON_EXTRACT_SET_004` | Generated deterministic SQLite parity case for JSON_EXTRACT_SET_004. |
| 1012 | P2 | memory | GEN_SQL_JSON | `JSON_EXTRACT_SET_005` | Generated deterministic SQLite parity case for JSON_EXTRACT_SET_005. |
| 1013 | P2 | memory | GEN_SQL_JSON | `JSON_EXTRACT_SET_006` | Generated deterministic SQLite parity case for JSON_EXTRACT_SET_006. |
| 1014 | P2 | memory | GEN_SQL_JSON | `JSON_EXTRACT_SET_007` | Generated deterministic SQLite parity case for JSON_EXTRACT_SET_007. |
| 1015 | P2 | memory | GEN_SQL_JSON | `JSON_EXTRACT_SET_008` | Generated deterministic SQLite parity case for JSON_EXTRACT_SET_008. |
| 1016 | P2 | memory | GEN_SQL_JSON | `JSON_EXTRACT_SET_009` | Generated deterministic SQLite parity case for JSON_EXTRACT_SET_009. |
| 1017 | P2 | memory | GEN_SQL_JSON | `JSON_EXTRACT_SET_010` | Generated deterministic SQLite parity case for JSON_EXTRACT_SET_010. |
| 1018 | P2 | memory | GEN_SQL_JSON | `JSON_EXTRACT_SET_011` | Generated deterministic SQLite parity case for JSON_EXTRACT_SET_011. |
| 1019 | P2 | memory | GEN_SQL_JSON | `JSON_EXTRACT_SET_012` | Generated deterministic SQLite parity case for JSON_EXTRACT_SET_012. |
| 1020 | P2 | memory | GEN_SQL_JSON | `JSON_EXTRACT_SET_013` | Generated deterministic SQLite parity case for JSON_EXTRACT_SET_013. |
| 1021 | P2 | memory | GEN_SQL_JSON | `JSON_EXTRACT_SET_014` | Generated deterministic SQLite parity case for JSON_EXTRACT_SET_014. |
| 1022 | P2 | memory | GEN_SQL_JSON | `JSON_EXTRACT_SET_015` | Generated deterministic SQLite parity case for JSON_EXTRACT_SET_015. |
| 1023 | P2 | memory | GEN_SQL_JSON | `JSON_EXTRACT_SET_016` | Generated deterministic SQLite parity case for JSON_EXTRACT_SET_016. |
| 1024 | P2 | memory | GEN_SQL_JSON | `JSON_EXTRACT_SET_017` | Generated deterministic SQLite parity case for JSON_EXTRACT_SET_017. |
| 1025 | P2 | memory | GEN_SQL_JSON | `JSON_EXTRACT_SET_018` | Generated deterministic SQLite parity case for JSON_EXTRACT_SET_018. |
| 1026 | P2 | memory | GEN_SQL_JSON | `JSON_EXTRACT_SET_019` | Generated deterministic SQLite parity case for JSON_EXTRACT_SET_019. |
| 1027 | P2 | memory | GEN_SQL_JSON | `JSON_EXTRACT_SET_020` | Generated deterministic SQLite parity case for JSON_EXTRACT_SET_020. |
| 1028 | P2 | memory | GEN_SQL_JSON | `JSON_EXTRACT_SET_021` | Generated deterministic SQLite parity case for JSON_EXTRACT_SET_021. |
| 1029 | P2 | memory | GEN_SQL_JSON | `JSON_EXTRACT_SET_022` | Generated deterministic SQLite parity case for JSON_EXTRACT_SET_022. |
| 1030 | P2 | memory | GEN_SQL_JSON | `JSON_EXTRACT_SET_023` | Generated deterministic SQLite parity case for JSON_EXTRACT_SET_023. |
| 1031 | P2 | memory | GEN_SQL_JSON | `JSON_EXTRACT_SET_024` | Generated deterministic SQLite parity case for JSON_EXTRACT_SET_024. |
| 1032 | P2 | memory | GEN_SQL_JSON | `JSON_EXTRACT_SET_025` | Generated deterministic SQLite parity case for JSON_EXTRACT_SET_025. |
| 1033 | P2 | memory | GEN_SQL_JSON | `JSON_EXTRACT_SET_026` | Generated deterministic SQLite parity case for JSON_EXTRACT_SET_026. |
| 1034 | P2 | memory | GEN_SQL_JSON | `JSON_EXTRACT_SET_027` | Generated deterministic SQLite parity case for JSON_EXTRACT_SET_027. |
| 1035 | P2 | memory | GEN_SQL_JSON | `JSON_EXTRACT_SET_028` | Generated deterministic SQLite parity case for JSON_EXTRACT_SET_028. |
| 1036 | P2 | memory | GEN_SQL_JSON | `JSON_EXTRACT_SET_029` | Generated deterministic SQLite parity case for JSON_EXTRACT_SET_029. |
| 1037 | P2 | memory | GEN_SQL_JSON | `JSON_EXTRACT_SET_030` | Generated deterministic SQLite parity case for JSON_EXTRACT_SET_030. |
| 1038 | P2 | memory | GEN_SQL_JSON | `JSON_EXTRACT_SET_031` | Generated deterministic SQLite parity case for JSON_EXTRACT_SET_031. |
| 1039 | P2 | memory | GEN_SQL_JSON | `JSON_EXTRACT_SET_032` | Generated deterministic SQLite parity case for JSON_EXTRACT_SET_032. |
| 1040 | P2 | memory | GEN_SQL_JSON | `JSON_EXTRACT_SET_033` | Generated deterministic SQLite parity case for JSON_EXTRACT_SET_033. |
| 1041 | P2 | memory | GEN_SQL_JSON | `JSON_EXTRACT_SET_034` | Generated deterministic SQLite parity case for JSON_EXTRACT_SET_034. |
| 1042 | P2 | memory | GEN_SQL_JSON | `JSON_EXTRACT_SET_035` | Generated deterministic SQLite parity case for JSON_EXTRACT_SET_035. |
| 1043 | P2 | memory | GEN_SQL_JSON | `JSON_EXTRACT_SET_036` | Generated deterministic SQLite parity case for JSON_EXTRACT_SET_036. |
| 1044 | P2 | memory | GEN_SQL_JSON | `JSON_EXTRACT_SET_037` | Generated deterministic SQLite parity case for JSON_EXTRACT_SET_037. |
| 1045 | P2 | memory | GEN_SQL_JSON | `JSON_EXTRACT_SET_038` | Generated deterministic SQLite parity case for JSON_EXTRACT_SET_038. |
| 1046 | P2 | memory | GEN_SQL_JSON | `JSON_EXTRACT_SET_039` | Generated deterministic SQLite parity case for JSON_EXTRACT_SET_039. |
| 1047 | P2 | memory | GEN_SQL_JSON | `JSON_EXTRACT_SET_040` | Generated deterministic SQLite parity case for JSON_EXTRACT_SET_040. |
| 1048 | P2 | memory | GEN_SQL_JSON | `JSON_EXTRACT_SET_041` | Generated deterministic SQLite parity case for JSON_EXTRACT_SET_041. |
| 1049 | P2 | memory | GEN_SQL_JSON | `JSON_EXTRACT_SET_042` | Generated deterministic SQLite parity case for JSON_EXTRACT_SET_042. |
| 1050 | P2 | memory | GEN_SQL_JSON | `JSON_EXTRACT_SET_043` | Generated deterministic SQLite parity case for JSON_EXTRACT_SET_043. |
| 1051 | P2 | memory | GEN_SQL_JSON | `JSON_EXTRACT_SET_044` | Generated deterministic SQLite parity case for JSON_EXTRACT_SET_044. |
| 1052 | P2 | memory | GEN_SQL_JSON | `JSON_EXTRACT_SET_045` | Generated deterministic SQLite parity case for JSON_EXTRACT_SET_045. |
| 1053 | P2 | memory | GEN_SQL_JSON | `JSON_EXTRACT_SET_046` | Generated deterministic SQLite parity case for JSON_EXTRACT_SET_046. |
| 1054 | P2 | memory | GEN_SQL_JSON | `JSON_EXTRACT_SET_047` | Generated deterministic SQLite parity case for JSON_EXTRACT_SET_047. |
| 1055 | P2 | memory | GEN_SQL_JSON | `JSON_EXTRACT_SET_048` | Generated deterministic SQLite parity case for JSON_EXTRACT_SET_048. |
| 1056 | P2 | memory | GEN_SQL_JSON | `JSON_EXTRACT_SET_049` | Generated deterministic SQLite parity case for JSON_EXTRACT_SET_049. |
| 1057 | P2 | memory | GEN_SQL_JSON | `JSON_EXTRACT_SET_050` | Generated deterministic SQLite parity case for JSON_EXTRACT_SET_050. |
| 1058 | P2 | memory | GEN_SQL_JSON | `JSON_EXTRACT_SET_051` | Generated deterministic SQLite parity case for JSON_EXTRACT_SET_051. |
| 1059 | P2 | memory | GEN_SQL_JSON | `JSON_EXTRACT_SET_052` | Generated deterministic SQLite parity case for JSON_EXTRACT_SET_052. |
| 1060 | P2 | memory | GEN_SQL_JSON | `JSON_EXTRACT_SET_053` | Generated deterministic SQLite parity case for JSON_EXTRACT_SET_053. |
| 1061 | P2 | memory | GEN_SQL_JSON | `JSON_EXTRACT_SET_054` | Generated deterministic SQLite parity case for JSON_EXTRACT_SET_054. |
| 1062 | P2 | memory | GEN_SQL_JSON | `JSON_EXTRACT_SET_055` | Generated deterministic SQLite parity case for JSON_EXTRACT_SET_055. |
| 1063 | P2 | memory | GEN_SQL_JSON | `JSON_EXTRACT_SET_056` | Generated deterministic SQLite parity case for JSON_EXTRACT_SET_056. |
| 1064 | P2 | memory | GEN_SQL_JSON | `JSON_EXTRACT_SET_057` | Generated deterministic SQLite parity case for JSON_EXTRACT_SET_057. |
| 1065 | P2 | memory | GEN_SQL_JSON | `JSON_EXTRACT_SET_058` | Generated deterministic SQLite parity case for JSON_EXTRACT_SET_058. |
| 1066 | P2 | memory | GEN_SQL_JSON | `JSON_EXTRACT_SET_059` | Generated deterministic SQLite parity case for JSON_EXTRACT_SET_059. |
| 1067 | P2 | memory | GEN_SQL_JSON | `JSON_EXTRACT_SET_060` | Generated deterministic SQLite parity case for JSON_EXTRACT_SET_060. |
| 1068 | P2 | memory | GEN_SQL_INDEX_PRAGMA | `INDEX_SCHEMA_PRAGMA_001` | Generated deterministic SQLite parity case for INDEX_SCHEMA_PRAGMA_001. |
| 1069 | P2 | memory | GEN_SQL_INDEX_PRAGMA | `INDEX_SCHEMA_PRAGMA_002` | Generated deterministic SQLite parity case for INDEX_SCHEMA_PRAGMA_002. |
| 1070 | P2 | memory | GEN_SQL_INDEX_PRAGMA | `INDEX_SCHEMA_PRAGMA_003` | Generated deterministic SQLite parity case for INDEX_SCHEMA_PRAGMA_003. |
| 1071 | P2 | memory | GEN_SQL_INDEX_PRAGMA | `INDEX_SCHEMA_PRAGMA_004` | Generated deterministic SQLite parity case for INDEX_SCHEMA_PRAGMA_004. |
| 1072 | P2 | memory | GEN_SQL_INDEX_PRAGMA | `INDEX_SCHEMA_PRAGMA_005` | Generated deterministic SQLite parity case for INDEX_SCHEMA_PRAGMA_005. |
| 1073 | P2 | memory | GEN_SQL_INDEX_PRAGMA | `INDEX_SCHEMA_PRAGMA_006` | Generated deterministic SQLite parity case for INDEX_SCHEMA_PRAGMA_006. |
| 1074 | P2 | memory | GEN_SQL_INDEX_PRAGMA | `INDEX_SCHEMA_PRAGMA_007` | Generated deterministic SQLite parity case for INDEX_SCHEMA_PRAGMA_007. |
| 1075 | P2 | memory | GEN_SQL_INDEX_PRAGMA | `INDEX_SCHEMA_PRAGMA_008` | Generated deterministic SQLite parity case for INDEX_SCHEMA_PRAGMA_008. |
| 1076 | P2 | memory | GEN_SQL_INDEX_PRAGMA | `INDEX_SCHEMA_PRAGMA_009` | Generated deterministic SQLite parity case for INDEX_SCHEMA_PRAGMA_009. |
| 1077 | P2 | memory | GEN_SQL_INDEX_PRAGMA | `INDEX_SCHEMA_PRAGMA_010` | Generated deterministic SQLite parity case for INDEX_SCHEMA_PRAGMA_010. |
| 1078 | P2 | memory | GEN_SQL_INDEX_PRAGMA | `INDEX_SCHEMA_PRAGMA_011` | Generated deterministic SQLite parity case for INDEX_SCHEMA_PRAGMA_011. |
| 1079 | P2 | memory | GEN_SQL_INDEX_PRAGMA | `INDEX_SCHEMA_PRAGMA_012` | Generated deterministic SQLite parity case for INDEX_SCHEMA_PRAGMA_012. |
| 1080 | P2 | memory | GEN_SQL_INDEX_PRAGMA | `INDEX_SCHEMA_PRAGMA_013` | Generated deterministic SQLite parity case for INDEX_SCHEMA_PRAGMA_013. |
| 1081 | P2 | memory | GEN_SQL_INDEX_PRAGMA | `INDEX_SCHEMA_PRAGMA_014` | Generated deterministic SQLite parity case for INDEX_SCHEMA_PRAGMA_014. |
| 1082 | P2 | memory | GEN_SQL_INDEX_PRAGMA | `INDEX_SCHEMA_PRAGMA_015` | Generated deterministic SQLite parity case for INDEX_SCHEMA_PRAGMA_015. |
| 1083 | P2 | memory | GEN_SQL_INDEX_PRAGMA | `INDEX_SCHEMA_PRAGMA_016` | Generated deterministic SQLite parity case for INDEX_SCHEMA_PRAGMA_016. |
| 1084 | P2 | memory | GEN_SQL_INDEX_PRAGMA | `INDEX_SCHEMA_PRAGMA_017` | Generated deterministic SQLite parity case for INDEX_SCHEMA_PRAGMA_017. |
| 1085 | P2 | memory | GEN_SQL_INDEX_PRAGMA | `INDEX_SCHEMA_PRAGMA_018` | Generated deterministic SQLite parity case for INDEX_SCHEMA_PRAGMA_018. |
| 1086 | P2 | memory | GEN_SQL_INDEX_PRAGMA | `INDEX_SCHEMA_PRAGMA_019` | Generated deterministic SQLite parity case for INDEX_SCHEMA_PRAGMA_019. |
| 1087 | P2 | memory | GEN_SQL_INDEX_PRAGMA | `INDEX_SCHEMA_PRAGMA_020` | Generated deterministic SQLite parity case for INDEX_SCHEMA_PRAGMA_020. |
| 1088 | P2 | memory | GEN_SQL_INDEX_PRAGMA | `INDEX_SCHEMA_PRAGMA_021` | Generated deterministic SQLite parity case for INDEX_SCHEMA_PRAGMA_021. |
| 1089 | P2 | memory | GEN_SQL_INDEX_PRAGMA | `INDEX_SCHEMA_PRAGMA_022` | Generated deterministic SQLite parity case for INDEX_SCHEMA_PRAGMA_022. |
| 1090 | P2 | memory | GEN_SQL_INDEX_PRAGMA | `INDEX_SCHEMA_PRAGMA_023` | Generated deterministic SQLite parity case for INDEX_SCHEMA_PRAGMA_023. |
| 1091 | P2 | memory | GEN_SQL_INDEX_PRAGMA | `INDEX_SCHEMA_PRAGMA_024` | Generated deterministic SQLite parity case for INDEX_SCHEMA_PRAGMA_024. |
| 1092 | P2 | memory | GEN_SQL_INDEX_PRAGMA | `INDEX_SCHEMA_PRAGMA_025` | Generated deterministic SQLite parity case for INDEX_SCHEMA_PRAGMA_025. |
| 1093 | P2 | memory | GEN_SQL_INDEX_PRAGMA | `INDEX_SCHEMA_PRAGMA_026` | Generated deterministic SQLite parity case for INDEX_SCHEMA_PRAGMA_026. |
| 1094 | P2 | memory | GEN_SQL_INDEX_PRAGMA | `INDEX_SCHEMA_PRAGMA_027` | Generated deterministic SQLite parity case for INDEX_SCHEMA_PRAGMA_027. |
| 1095 | P2 | memory | GEN_SQL_INDEX_PRAGMA | `INDEX_SCHEMA_PRAGMA_028` | Generated deterministic SQLite parity case for INDEX_SCHEMA_PRAGMA_028. |
| 1096 | P2 | memory | GEN_SQL_INDEX_PRAGMA | `INDEX_SCHEMA_PRAGMA_029` | Generated deterministic SQLite parity case for INDEX_SCHEMA_PRAGMA_029. |
| 1097 | P2 | memory | GEN_SQL_INDEX_PRAGMA | `INDEX_SCHEMA_PRAGMA_030` | Generated deterministic SQLite parity case for INDEX_SCHEMA_PRAGMA_030. |
| 1098 | P2 | memory | GEN_SQL_INDEX_PRAGMA | `INDEX_SCHEMA_PRAGMA_031` | Generated deterministic SQLite parity case for INDEX_SCHEMA_PRAGMA_031. |
| 1099 | P2 | memory | GEN_SQL_INDEX_PRAGMA | `INDEX_SCHEMA_PRAGMA_032` | Generated deterministic SQLite parity case for INDEX_SCHEMA_PRAGMA_032. |
| 1100 | P2 | memory | GEN_SQL_INDEX_PRAGMA | `INDEX_SCHEMA_PRAGMA_033` | Generated deterministic SQLite parity case for INDEX_SCHEMA_PRAGMA_033. |
| 1101 | P2 | memory | GEN_SQL_INDEX_PRAGMA | `INDEX_SCHEMA_PRAGMA_034` | Generated deterministic SQLite parity case for INDEX_SCHEMA_PRAGMA_034. |
| 1102 | P2 | memory | GEN_SQL_INDEX_PRAGMA | `INDEX_SCHEMA_PRAGMA_035` | Generated deterministic SQLite parity case for INDEX_SCHEMA_PRAGMA_035. |
| 1103 | P2 | memory | GEN_SQL_INDEX_PRAGMA | `INDEX_SCHEMA_PRAGMA_036` | Generated deterministic SQLite parity case for INDEX_SCHEMA_PRAGMA_036. |
| 1104 | P2 | memory | GEN_SQL_INDEX_PRAGMA | `INDEX_SCHEMA_PRAGMA_037` | Generated deterministic SQLite parity case for INDEX_SCHEMA_PRAGMA_037. |
| 1105 | P2 | memory | GEN_SQL_INDEX_PRAGMA | `INDEX_SCHEMA_PRAGMA_038` | Generated deterministic SQLite parity case for INDEX_SCHEMA_PRAGMA_038. |
| 1106 | P2 | memory | GEN_SQL_INDEX_PRAGMA | `INDEX_SCHEMA_PRAGMA_039` | Generated deterministic SQLite parity case for INDEX_SCHEMA_PRAGMA_039. |
| 1107 | P2 | memory | GEN_SQL_INDEX_PRAGMA | `INDEX_SCHEMA_PRAGMA_040` | Generated deterministic SQLite parity case for INDEX_SCHEMA_PRAGMA_040. |
| 1108 | P2 | memory | GEN_SQL_INDEX_PRAGMA | `INDEX_SCHEMA_PRAGMA_041` | Generated deterministic SQLite parity case for INDEX_SCHEMA_PRAGMA_041. |
| 1109 | P2 | memory | GEN_SQL_INDEX_PRAGMA | `INDEX_SCHEMA_PRAGMA_042` | Generated deterministic SQLite parity case for INDEX_SCHEMA_PRAGMA_042. |
| 1110 | P2 | memory | GEN_SQL_INDEX_PRAGMA | `INDEX_SCHEMA_PRAGMA_043` | Generated deterministic SQLite parity case for INDEX_SCHEMA_PRAGMA_043. |
| 1111 | P2 | memory | GEN_SQL_INDEX_PRAGMA | `INDEX_SCHEMA_PRAGMA_044` | Generated deterministic SQLite parity case for INDEX_SCHEMA_PRAGMA_044. |
| 1112 | P2 | memory | GEN_SQL_INDEX_PRAGMA | `INDEX_SCHEMA_PRAGMA_045` | Generated deterministic SQLite parity case for INDEX_SCHEMA_PRAGMA_045. |
| 1113 | P2 | memory | GEN_SQL_INDEX_PRAGMA | `INDEX_SCHEMA_PRAGMA_046` | Generated deterministic SQLite parity case for INDEX_SCHEMA_PRAGMA_046. |
| 1114 | P2 | memory | GEN_SQL_INDEX_PRAGMA | `INDEX_SCHEMA_PRAGMA_047` | Generated deterministic SQLite parity case for INDEX_SCHEMA_PRAGMA_047. |
| 1115 | P2 | memory | GEN_SQL_INDEX_PRAGMA | `INDEX_SCHEMA_PRAGMA_048` | Generated deterministic SQLite parity case for INDEX_SCHEMA_PRAGMA_048. |
| 1116 | P2 | memory | GEN_SQL_INDEX_PRAGMA | `INDEX_SCHEMA_PRAGMA_049` | Generated deterministic SQLite parity case for INDEX_SCHEMA_PRAGMA_049. |
| 1117 | P2 | memory | GEN_SQL_INDEX_PRAGMA | `INDEX_SCHEMA_PRAGMA_050` | Generated deterministic SQLite parity case for INDEX_SCHEMA_PRAGMA_050. |
| 1118 | P2 | memory | GEN_SQL_INDEX_PRAGMA | `INDEX_SCHEMA_PRAGMA_051` | Generated deterministic SQLite parity case for INDEX_SCHEMA_PRAGMA_051. |
| 1119 | P2 | memory | GEN_SQL_INDEX_PRAGMA | `INDEX_SCHEMA_PRAGMA_052` | Generated deterministic SQLite parity case for INDEX_SCHEMA_PRAGMA_052. |
| 1120 | P2 | memory | GEN_SQL_INDEX_PRAGMA | `INDEX_SCHEMA_PRAGMA_053` | Generated deterministic SQLite parity case for INDEX_SCHEMA_PRAGMA_053. |
| 1121 | P2 | memory | GEN_SQL_INDEX_PRAGMA | `INDEX_SCHEMA_PRAGMA_054` | Generated deterministic SQLite parity case for INDEX_SCHEMA_PRAGMA_054. |
| 1122 | P2 | memory | GEN_SQL_INDEX_PRAGMA | `INDEX_SCHEMA_PRAGMA_055` | Generated deterministic SQLite parity case for INDEX_SCHEMA_PRAGMA_055. |
| 1123 | P2 | memory | GEN_SQL_INDEX_PRAGMA | `INDEX_SCHEMA_PRAGMA_056` | Generated deterministic SQLite parity case for INDEX_SCHEMA_PRAGMA_056. |
| 1124 | P2 | memory | GEN_SQL_INDEX_PRAGMA | `INDEX_SCHEMA_PRAGMA_057` | Generated deterministic SQLite parity case for INDEX_SCHEMA_PRAGMA_057. |
| 1125 | P2 | memory | GEN_SQL_INDEX_PRAGMA | `INDEX_SCHEMA_PRAGMA_058` | Generated deterministic SQLite parity case for INDEX_SCHEMA_PRAGMA_058. |
| 1126 | P2 | memory | GEN_SQL_INDEX_PRAGMA | `INDEX_SCHEMA_PRAGMA_059` | Generated deterministic SQLite parity case for INDEX_SCHEMA_PRAGMA_059. |
| 1127 | P2 | memory | GEN_SQL_INDEX_PRAGMA | `INDEX_SCHEMA_PRAGMA_060` | Generated deterministic SQLite parity case for INDEX_SCHEMA_PRAGMA_060. |
