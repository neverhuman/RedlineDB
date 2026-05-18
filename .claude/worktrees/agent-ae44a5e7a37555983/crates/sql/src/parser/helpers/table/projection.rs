use crate::statement::SelectSource;

pub(crate) fn push_projection_columns(source: &SelectSource, out: &mut Vec<String>) {
    match source {
        SelectSource::Table(table) => {
            out.extend(table.columns.iter().map(|column| column.name.to_string()));
        }
        SelectSource::Tables(tables) => {
            for table in tables {
                out.extend(table.table.columns.iter().map(|column| {
                    if let Some(alias) = &table.alias {
                        format!("{}.{}", alias, column.name)
                    } else {
                        format!("{}.{}", table.table.name, column.name)
                    }
                }));
            }
        }
        SelectSource::Joined(join) => {
            out.extend(join.base.table.columns.iter().map(|column| {
                if let Some(alias) = &join.base.alias {
                    format!("{}.{}", alias, column.name)
                } else {
                    format!("{}.{}", join.base.table.name, column.name)
                }
            }));
            for step in &join.joins {
                out.extend(step.right.table.columns.iter().map(|column| {
                    if let Some(alias) = &step.right.alias {
                        format!("{}.{}", alias, column.name)
                    } else {
                        format!("{}.{}", step.right.table.name, column.name)
                    }
                }));
            }
        }
        SelectSource::SqliteSchema => {
            out.extend(
                ["type", "name", "tbl_name", "rootpage", "sql"]
                    .into_iter()
                    .map(str::to_owned),
            );
        }
        SelectSource::StaticRows { .. } => {}
        SelectSource::CompoundAll(_) => {}
        SelectSource::CompoundSet { .. } => {}
        SelectSource::Cte {
            name,
            alias,
            columns,
            ..
        } => {
            let prefix = alias.as_deref().unwrap_or(name.as_ref());
            out.extend(columns.iter().map(|c| format!("{prefix}.{c}")));
        }
        SelectSource::Empty => {}
    }
}

pub(crate) fn render_expr_name(expr: &sqlparser::ast::Expr) -> String {
    match expr {
        sqlparser::ast::Expr::Identifier(ident) => ident.value.clone(),
        sqlparser::ast::Expr::CompoundIdentifier(parts) => {
            match parts.last().map(|ident| ident.value.clone()) {
                Some(name) => name,
                None => expr.to_string(),
            }
        }
        _ => expr.to_string(),
    }
}

/// Best-effort derivation of output column names for a leaf `SelectPlan`,
/// used by the compound-SELECT executor so that an `ORDER BY <col>` clause
/// at the compound level can resolve identifiers against the synthesised
/// compound row.
///
/// Handles three cases: explicit projection (use alias or rendered expr
/// name), wildcard projection (delegate to `push_projection_columns`),
/// and an empty projection where the source is itself another compound
/// (recurse into the first branch).
pub(crate) fn select_plan_output_names(plan: &crate::statement::SelectPlan) -> Vec<String> {
    use crate::statement::SelectSource;
    use sqlparser::ast::SelectItem;
    let mut out = Vec::new();
    if plan.projection.is_empty() {
        // Empty projection means the plan emits the source's columns.
        // For nested compounds, recurse into the first branch.
        match &plan.source {
            SelectSource::CompoundAll(branches)
            | SelectSource::CompoundSet { branches, .. } => {
                if let Some(first) = branches.first() {
                    return select_plan_output_names(first);
                }
            }
            _ => {}
        }
        push_projection_columns(&plan.source, &mut out);
        // Strip qualifier — compound rows are positional so the
        // unqualified suffix is what ORDER BY identifiers will match.
        return out
            .into_iter()
            .map(|name| match name.rsplit_once('.') {
                Some((_, tail)) => tail.to_owned(),
                None => name,
            })
            .collect();
    }
    for item in &plan.projection {
        match item {
            SelectItem::UnnamedExpr(expr) => out.push(render_expr_name(expr)),
            SelectItem::ExprWithAlias { alias, .. } => out.push(alias.value.clone()),
            SelectItem::Wildcard(_) | SelectItem::QualifiedWildcard(_, _) => {
                let mut wide = Vec::new();
                push_projection_columns(&plan.source, &mut wide);
                for n in wide {
                    out.push(match n.rsplit_once('.') {
                        Some((_, tail)) => tail.to_owned(),
                        None => n,
                    });
                }
            }
        }
    }
    out
}
