//! A6 SQLite parity — foreign-key enforcement.
//!
//! Each test runs the same SQL script against `rusqlite` and RedlineDB
//! and asserts identical `(rows-normalized, error-class)`. The harness
//! lives at `crates/sql/tests/parity_oracle/harness.rs`; we reuse it via
//! `#[path = "..."]` like the other parity-* suites.

#![allow(clippy::needless_borrow)]

#[path = "parity_oracle/harness.rs"]
mod harness;

use harness::assert_parity;

// -- Immediate enforcement: INSERT -------------------------------------------

#[test]
fn insert_into_child_with_existing_parent_succeeds() {
    assert_parity(
        "PRAGMA foreign_keys = ON;\
         CREATE TABLE parent(id INTEGER PRIMARY KEY);\
         CREATE TABLE child(\
             id INTEGER PRIMARY KEY,\
             pid INTEGER REFERENCES parent(id)\
         );\
         INSERT INTO parent VALUES (1);\
         INSERT INTO child VALUES (10, 1);\
         SELECT id, pid FROM child ORDER BY id",
    );
}

#[test]
fn insert_into_child_with_missing_parent_errors() {
    assert_parity(
        "PRAGMA foreign_keys = ON;\
         CREATE TABLE parent(id INTEGER PRIMARY KEY);\
         CREATE TABLE child(\
             id INTEGER PRIMARY KEY,\
             pid INTEGER REFERENCES parent(id)\
         );\
         INSERT INTO child VALUES (10, 999);\
         SELECT id, pid FROM child ORDER BY id",
    );
}

#[test]
fn insert_with_null_fk_value_is_allowed() {
    // SQLite default MATCH SIMPLE: NULL FK exempts the row.
    assert_parity(
        "PRAGMA foreign_keys = ON;\
         CREATE TABLE parent(id INTEGER PRIMARY KEY);\
         CREATE TABLE child(\
             id INTEGER PRIMARY KEY,\
             pid INTEGER REFERENCES parent(id)\
         );\
         INSERT INTO child VALUES (10, NULL);\
         SELECT id, pid FROM child ORDER BY id",
    );
}

// -- Immediate enforcement: UPDATE -------------------------------------------

#[test]
fn update_child_fk_to_existing_parent_succeeds() {
    assert_parity(
        "PRAGMA foreign_keys = ON;\
         CREATE TABLE parent(id INTEGER PRIMARY KEY);\
         CREATE TABLE child(\
             id INTEGER PRIMARY KEY,\
             pid INTEGER REFERENCES parent(id)\
         );\
         INSERT INTO parent VALUES (1), (2);\
         INSERT INTO child VALUES (10, 1);\
         UPDATE child SET pid = 2 WHERE id = 10;\
         SELECT id, pid FROM child ORDER BY id",
    );
}

#[test]
fn update_child_fk_to_missing_parent_errors() {
    assert_parity(
        "PRAGMA foreign_keys = ON;\
         CREATE TABLE parent(id INTEGER PRIMARY KEY);\
         CREATE TABLE child(\
             id INTEGER PRIMARY KEY,\
             pid INTEGER REFERENCES parent(id)\
         );\
         INSERT INTO parent VALUES (1);\
         INSERT INTO child VALUES (10, 1);\
         UPDATE child SET pid = 7 WHERE id = 10;\
         SELECT id, pid FROM child ORDER BY id",
    );
}

// -- ON DELETE NO ACTION (SQLite default) ------------------------------------

#[test]
fn delete_parent_with_referencing_child_errors_by_default() {
    assert_parity(
        "PRAGMA foreign_keys = ON;\
         CREATE TABLE parent(id INTEGER PRIMARY KEY);\
         CREATE TABLE child(\
             id INTEGER PRIMARY KEY,\
             pid INTEGER REFERENCES parent(id)\
         );\
         INSERT INTO parent VALUES (1);\
         INSERT INTO child VALUES (10, 1);\
         DELETE FROM parent WHERE id = 1;\
         SELECT id FROM parent ORDER BY id",
    );
}

// -- ON DELETE RESTRICT ------------------------------------------------------

#[test]
fn delete_parent_restrict_errors() {
    assert_parity(
        "PRAGMA foreign_keys = ON;\
         CREATE TABLE parent(id INTEGER PRIMARY KEY);\
         CREATE TABLE child(\
             id INTEGER PRIMARY KEY,\
             pid INTEGER REFERENCES parent(id) ON DELETE RESTRICT\
         );\
         INSERT INTO parent VALUES (1);\
         INSERT INTO child VALUES (10, 1);\
         DELETE FROM parent WHERE id = 1;\
         SELECT id FROM parent ORDER BY id",
    );
}

// -- ON DELETE CASCADE -------------------------------------------------------

#[test]
fn delete_parent_cascade_removes_children() {
    assert_parity(
        "PRAGMA foreign_keys = ON;\
         CREATE TABLE parent(id INTEGER PRIMARY KEY);\
         CREATE TABLE child(\
             id INTEGER PRIMARY KEY,\
             pid INTEGER REFERENCES parent(id) ON DELETE CASCADE\
         );\
         INSERT INTO parent VALUES (1), (2);\
         INSERT INTO child VALUES (10, 1), (11, 1), (12, 2);\
         DELETE FROM parent WHERE id = 1;\
         SELECT id, pid FROM child ORDER BY id",
    );
}

// -- ON DELETE SET NULL ------------------------------------------------------

#[test]
fn delete_parent_set_null_nulls_children() {
    assert_parity(
        "PRAGMA foreign_keys = ON;\
         CREATE TABLE parent(id INTEGER PRIMARY KEY);\
         CREATE TABLE child(\
             id INTEGER PRIMARY KEY,\
             pid INTEGER REFERENCES parent(id) ON DELETE SET NULL\
         );\
         INSERT INTO parent VALUES (1);\
         INSERT INTO child VALUES (10, 1);\
         DELETE FROM parent WHERE id = 1;\
         SELECT id, pid FROM child ORDER BY id",
    );
}

// -- ON DELETE SET DEFAULT ---------------------------------------------------

#[test]
fn delete_parent_set_default_substitutes_default() {
    assert_parity(
        "PRAGMA foreign_keys = ON;\
         CREATE TABLE parent(id INTEGER PRIMARY KEY);\
         CREATE TABLE child(\
             id INTEGER PRIMARY KEY,\
             pid INTEGER DEFAULT 0 REFERENCES parent(id) ON DELETE SET DEFAULT\
         );\
         INSERT INTO parent VALUES (1);\
         INSERT INTO child VALUES (10, 1);\
         DELETE FROM parent WHERE id = 1;\
         SELECT id, pid FROM child ORDER BY id",
    );
}

// -- ON UPDATE CASCADE -------------------------------------------------------

#[test]
fn update_parent_cascade_propagates_to_children() {
    assert_parity(
        "PRAGMA foreign_keys = ON;\
         CREATE TABLE parent(id INTEGER PRIMARY KEY);\
         CREATE TABLE child(\
             id INTEGER PRIMARY KEY,\
             pid INTEGER REFERENCES parent(id) ON UPDATE CASCADE\
         );\
         INSERT INTO parent VALUES (1);\
         INSERT INTO child VALUES (10, 1);\
         UPDATE parent SET id = 7 WHERE id = 1;\
         SELECT id, pid FROM child ORDER BY id",
    );
}

#[test]
fn update_parent_set_null_nulls_children() {
    assert_parity(
        "PRAGMA foreign_keys = ON;\
         CREATE TABLE parent(id INTEGER PRIMARY KEY);\
         CREATE TABLE child(\
             id INTEGER PRIMARY KEY,\
             pid INTEGER REFERENCES parent(id) ON UPDATE SET NULL\
         );\
         INSERT INTO parent VALUES (1);\
         INSERT INTO child VALUES (10, 1);\
         UPDATE parent SET id = 99 WHERE id = 1;\
         SELECT id, pid FROM child ORDER BY id",
    );
}

// -- Table-level FOREIGN KEY syntax ------------------------------------------

#[test]
fn table_level_fk_enforces_inserts() {
    assert_parity(
        "PRAGMA foreign_keys = ON;\
         CREATE TABLE parent(id INTEGER PRIMARY KEY);\
         CREATE TABLE child(\
             id INTEGER PRIMARY KEY,\
             pid INTEGER,\
             FOREIGN KEY(pid) REFERENCES parent(id)\
         );\
         INSERT INTO child VALUES (10, 99);\
         SELECT id, pid FROM child",
    );
}

// -- PRAGMA toggle behaviour -------------------------------------------------

#[test]
fn fk_disabled_via_explicit_pragma_skips_checks() {
    // SQLite's bundled default for FK enforcement is undefined across
    // versions; pin the test with an explicit PRAGMA OFF to assert the
    // parity gate without depending on the harness default.
    assert_parity(
        "PRAGMA foreign_keys = OFF;\
         CREATE TABLE parent(id INTEGER PRIMARY KEY);\
         CREATE TABLE child(\
             id INTEGER PRIMARY KEY,\
             pid INTEGER REFERENCES parent(id)\
         );\
         INSERT INTO child VALUES (10, 999);\
         SELECT id, pid FROM child ORDER BY id",
    );
}

#[test]
fn pragma_foreign_keys_off_skips_enforcement() {
    assert_parity(
        "PRAGMA foreign_keys = ON;\
         PRAGMA foreign_keys = OFF;\
         CREATE TABLE parent(id INTEGER PRIMARY KEY);\
         CREATE TABLE child(\
             id INTEGER PRIMARY KEY,\
             pid INTEGER REFERENCES parent(id)\
         );\
         INSERT INTO child VALUES (10, 7);\
         SELECT id, pid FROM child ORDER BY id",
    );
}

// -- DEFERRABLE INITIALLY DEFERRED -------------------------------------------

#[test]
fn deferred_fk_inside_tx_allows_temporary_violation() {
    // SQLite under DEFERRED+ON: child can be inserted ahead of its parent
    // within the same transaction, provided the parent row exists by COMMIT.
    assert_parity(
        "PRAGMA foreign_keys = ON;\
         CREATE TABLE parent(id INTEGER PRIMARY KEY);\
         CREATE TABLE child(\
             id INTEGER PRIMARY KEY,\
             pid INTEGER,\
             FOREIGN KEY(pid) REFERENCES parent(id) DEFERRABLE INITIALLY DEFERRED\
         );\
         BEGIN;\
         INSERT INTO child VALUES (10, 1);\
         INSERT INTO parent VALUES (1);\
         COMMIT;\
         SELECT id, pid FROM child ORDER BY id",
    );
}

#[test]
fn deferred_fk_unresolved_at_commit_errors() {
    assert_parity(
        "PRAGMA foreign_keys = ON;\
         CREATE TABLE parent(id INTEGER PRIMARY KEY);\
         CREATE TABLE child(\
             id INTEGER PRIMARY KEY,\
             pid INTEGER,\
             FOREIGN KEY(pid) REFERENCES parent(id) DEFERRABLE INITIALLY DEFERRED\
         );\
         BEGIN;\
         INSERT INTO child VALUES (10, 1);\
         COMMIT;\
         SELECT id, pid FROM child ORDER BY id",
    );
}
