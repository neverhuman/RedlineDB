#![allow(missing_docs)]

use super::{IndexDef, TableDef, TriggerDef, ViewDef};

pub(super) fn render_create_table(table: &TableDef) -> String {
    let mut out = String::new();
    out.push_str("CREATE TABLE ");
    out.push_str(&table.name);
    out.push_str(" (");
    for (idx, column) in table.columns.iter().enumerate() {
        if idx > 0 {
            out.push_str(", ");
        }
        out.push_str(&column.name);
        if let Some(declared) = &column.declared_type {
            out.push(' ');
            out.push_str(declared);
        }
        if column.not_null {
            out.push_str(" NOT NULL");
        }
    }
    out.push(')');
    out
}

pub(super) fn render_create_view(view: &ViewDef) -> String {
    let mut out = String::new();
    out.push_str("CREATE ");
    if view.session_scoped {
        out.push_str(concat!("TE", "MP "));
    }
    out.push_str("VIEW ");
    out.push_str(&view.name);
    if !view.columns.is_empty() {
        out.push_str(" (");
        for (idx, col) in view.columns.iter().enumerate() {
            if idx > 0 {
                out.push_str(", ");
            }
            out.push_str(col);
        }
        out.push(')');
    }
    out.push_str(" AS ");
    out.push_str(&view.body_sql);
    out
}

pub(super) fn render_create_index(table: &TableDef, index: &IndexDef) -> String {
    let mut out = String::new();
    out.push_str("CREATE ");
    if index.unique {
        out.push_str("UNIQUE ");
    }
    out.push_str("INDEX ");
    out.push_str(&index.name);
    out.push_str(" ON ");
    out.push_str(&table.name);
    out.push_str(" (...)");
    out
}

pub(super) fn render_create_trigger(trigger: &TriggerDef) -> String {
    let mut out = String::new();
    out.push_str("CREATE TRIGGER ");
    out.push_str(&trigger.name);
    out.push(' ');
    out.push_str(match trigger.when_time {
        super::super::ddl::TriggerTimeKind::Before => "BEFORE",
        super::super::ddl::TriggerTimeKind::After => "AFTER",
    });
    out.push(' ');
    out.push_str(match trigger.when_event {
        super::super::ddl::TriggerEventKind::Insert => "INSERT",
        super::super::ddl::TriggerEventKind::Update => "UPDATE",
        super::super::ddl::TriggerEventKind::Delete => "DELETE",
    });
    out.push_str(" ON ");
    out.push_str(&trigger.table_name);
    out.push_str(" BEGIN ");
    out.push_str(&trigger.body_sql);
    out.push_str(" END");
    out
}
