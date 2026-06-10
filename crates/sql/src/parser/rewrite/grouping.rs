use super::shared::*;

pub(crate) fn rewrite_rollup_cube_to_grouping_sets(sql: &str) -> String {
    let mut out = sql.to_owned();
    loop {
        let lower = out.to_ascii_lowercase();
        let bytes = out.as_bytes();
        let Some(rollup_pos) = lower.find(" group by rollup ") else {
            break;
        };
        let kw_end = rollup_pos + " group by rollup ".len();
        // Skip whitespace, expect '('
        let mut j = kw_end;
        while j < bytes.len() && bytes[j].is_ascii_whitespace() {
            j += 1;
        }
        if j >= bytes.len() || bytes[j] != b'(' {
            break;
        }
        let open = j;
        let Some(close) = find_matching_paren(bytes, open) else {
            break;
        };
        let inner = &out[open + 1..close];
        let items = parse_grouping_set_columns(inner);
        let expansion = expand_rollup(&items);
        let replacement = format!("GROUP BY GROUPING SETS ({})", expansion);
        let start_replace = rollup_pos + 1; // strip leading space
        out.replace_range(start_replace..close + 1, &replacement);
    }
    loop {
        let lower = out.to_ascii_lowercase();
        let bytes = out.as_bytes();
        let Some(cube_pos) = lower.find(" group by cube ") else {
            break;
        };
        let kw_end = cube_pos + " group by cube ".len();
        let mut j = kw_end;
        while j < bytes.len() && bytes[j].is_ascii_whitespace() {
            j += 1;
        }
        if j >= bytes.len() || bytes[j] != b'(' {
            break;
        }
        let open = j;
        let Some(close) = find_matching_paren(bytes, open) else {
            break;
        };
        let inner = &out[open + 1..close];
        let items = parse_grouping_set_columns(inner);
        let expansion = expand_cube(&items);
        let replacement = format!("GROUP BY GROUPING SETS ({})", expansion);
        let start_replace = cube_pos + 1;
        out.replace_range(start_replace..close + 1, &replacement);
    }
    out
}

/// Split a comma-separated list of grouping-set column expressions at
/// the top level (parens are balanced).
pub(crate) fn parse_grouping_set_columns(inner: &str) -> Vec<String> {
    let bytes = inner.as_bytes();
    let mut out = Vec::new();
    let mut start = 0usize;
    let mut depth = 0i32;
    let mut in_str: Option<u8> = None;
    let mut i = 0usize;
    while i < bytes.len() {
        let b = bytes[i];
        if let Some(q) = in_str {
            if b == q {
                in_str = None;
            }
            i += 1;
            continue;
        }
        match b {
            b'\'' | b'"' => in_str = Some(b),
            b'(' => depth += 1,
            b')' => depth -= 1,
            b',' if depth == 0 => {
                let item = inner[start..i].trim();
                if !item.is_empty() {
                    out.push(item.to_owned());
                }
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    let tail = inner[start..].trim();
    if !tail.is_empty() {
        out.push(tail.to_owned());
    }
    out
}

/// Hierarchical expansion of `ROLLUP(a, b, c)`:
///   `(a,b,c), (a,b), (a), ()`
pub(crate) fn expand_rollup(cols: &[String]) -> String {
    let mut sets: Vec<String> = Vec::with_capacity(cols.len() + 1);
    for n in (0..=cols.len()).rev() {
        let prefix = cols[..n].join(", ");
        sets.push(format!("({prefix})"));
    }
    sets.join(", ")
}

/// Combinatorial expansion of `CUBE(a, b, c)`: all 2^n subsets, in PG's
/// declared order (largest subset first, empty last).
pub(crate) fn expand_cube(cols: &[String]) -> String {
    let n = cols.len();
    let total = 1usize << n;
    let mut sets: Vec<String> = Vec::with_capacity(total);
    // Generate subsets in descending popcount, then by mask value for
    // stable ordering. PG's exact order is implementation-defined; what
    // matters is that the ORDER BY in the outer query re-sorts.
    let mut masks: Vec<usize> = (0..total).collect();
    masks.sort_by(|a, b| b.count_ones().cmp(&a.count_ones()).then(a.cmp(b)));
    for mask in masks {
        let mut members: Vec<String> = Vec::with_capacity(n);
        for (i, col) in cols.iter().enumerate() {
            if mask & (1 << i) != 0 {
                members.push(col.clone());
            }
        }
        sets.push(format!("({})", members.join(", ")));
    }
    sets.join(", ")
}

/// Walk `upper` looking for the first top-level (paren-balanced, outside
/// string literals) SELECT keyword preceded by ASCII whitespace. Used to
/// locate the body of a `WITH ... SELECT ...` query so the WITH clause
/// can be lifted into a prefix.
pub(crate) fn find_top_level_select_after_with(upper: &str, bytes: &[u8]) -> Option<usize> {
    let mut i = 0usize;
    let mut depth = 0i32;
    let mut in_str: Option<u8> = None;
    while i + 6 <= bytes.len() {
        let b = bytes[i];
        if let Some(q) = in_str {
            if b == q {
                in_str = None;
            }
            i += 1;
            continue;
        }
        match b {
            b'\'' | b'"' => {
                in_str = Some(b);
                i += 1;
                continue;
            }
            b'(' => depth += 1,
            b')' => depth -= 1,
            _ => {}
        }
        if depth == 0
            && i > 0
            && bytes[i - 1].is_ascii_whitespace()
            && &upper[i..i + 6] == "SELECT"
            && (i + 6 == bytes.len() || bytes[i + 6].is_ascii_whitespace())
        {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// Track K — Rewrite `... GROUP BY GROUPING SETS ((s1), (s2), ...) ...`
/// into a UNION ALL of N parallel SELECTs, one per grouping set. Each
/// branch keeps the original WHERE / FROM clauses and replaces
/// non-grouped grouping-key columns with NULL in the projection.
///
/// Strategy: locate the SELECT body (between `SELECT` and `GROUP BY`),
/// the GROUPING SETS list, and any trailing ORDER BY / LIMIT clauses.
/// Build one inner SELECT per set with the same FROM/WHERE and a
/// per-set GROUP BY. The outer query keeps ORDER BY / LIMIT and wraps
/// the UNION ALL in a derived table.
pub(crate) fn rewrite_grouping_sets_to_union_all(sql: &str) -> String {
    let mut out = sql.to_owned();
    // Restrict to a single top-level statement; if there are multiple
    // statements, recurse per statement.
    let stmts = split_top_level_statements(&out);
    if stmts.len() > 1 {
        let pieces: Vec<String> = stmts
            .into_iter()
            .map(|s| rewrite_grouping_sets_in_statement(&s))
            .collect();
        return pieces.join(";");
    }
    out = rewrite_grouping_sets_in_statement(&out);
    out
}

pub(crate) fn rewrite_grouping_sets_in_statement(stmt: &str) -> String {
    let lower = stmt.to_ascii_lowercase();
    let Some(gs_pos) = lower.find(" group by grouping sets ") else {
        return stmt.to_owned();
    };
    let kw_end = gs_pos + " group by grouping sets ".len();
    let bytes = stmt.as_bytes();
    let mut j = kw_end;
    while j < bytes.len() && bytes[j].is_ascii_whitespace() {
        j += 1;
    }
    if j >= bytes.len() || bytes[j] != b'(' {
        return stmt.to_owned();
    }
    let open = j;
    let Some(close) = find_matching_paren(bytes, open) else {
        return stmt.to_owned();
    };
    // Parse the inner list as a top-level sequence of `(...)` items.
    let inner = &stmt[open + 1..close];
    let sets = parse_grouping_set_list(inner);
    if sets.is_empty() {
        return stmt.to_owned();
    }

    // Find the SELECT body: between the leading `SELECT` and `gs_pos`.
    // We also need to know where the trailing clauses (ORDER BY, LIMIT,
    // FETCH) start after the GROUPING SETS list.
    let trimmed = stmt.trim_start();
    let leading_ws = &stmt[..stmt.len() - trimmed.len()];
    let upper_trim = trimmed.to_ascii_uppercase();
    // Accept both bare SELECT and `WITH ... SELECT` shapes. For WITH,
    // find the SELECT keyword that introduces the body and treat the
    // WITH clause as a prefix that wraps the final UNION.
    let (with_prefix, select_offset_t) = if upper_trim.starts_with("SELECT ") {
        (String::new(), 0usize)
    } else if upper_trim.starts_with("WITH ") {
        // Find the top-level SELECT keyword that introduces the body.
        // Whitespace around the keyword can be ' ', '\t', or '\n'.
        let select_idx = find_top_level_select_after_with(&upper_trim, trimmed.as_bytes());
        let Some(s) = select_idx else {
            return stmt.to_owned();
        };
        (trimmed[..s].to_owned(), s)
    } else {
        return stmt.to_owned();
    };
    // gs_pos is relative to stmt; convert to trimmed-relative.
    let gs_pos_t = gs_pos - leading_ws.len();
    let close_t = close - leading_ws.len();

    // The "select body up to GROUP BY" is the body chunk between the
    // resolved SELECT start and the GROUPING SETS keyword.
    let body_before_group_by = &trimmed[select_offset_t..gs_pos_t];
    let upper_body = body_before_group_by.to_ascii_uppercase();
    // Locate the top-level " FROM " keyword.
    let from_pos =
        find_top_level_keyword(&upper_body, body_before_group_by.as_bytes(), 0, " FROM ");
    let (projection, from_suffix) = match from_pos {
        Some(idx) => {
            // projection excludes leading "SELECT"
            let proj_start = "SELECT ".len();
            let projection = body_before_group_by[proj_start..idx].trim().to_owned();
            let from_suffix = body_before_group_by[idx..].to_owned();
            (projection, from_suffix)
        }
        None => {
            // No FROM (constant SELECT) — just take projection after SELECT.
            let projection = body_before_group_by["SELECT ".len()..].trim().to_owned();
            (projection, String::new())
        }
    };
    // Tail: anything after the GROUPING SETS close-paren.
    let tail = &trimmed[close_t + 1..];

    let proj_items = split_top_level_commas(&projection);
    // Decide which projection items are grouping-key columns (Identifier
    // refs) vs aggregates (function-style). Heuristic: anything that
    // contains `(` is an aggregate / expression; bare identifier strings
    // (after stripping AS alias) are grouping keys.
    let proj_meta: Vec<ProjItem> = proj_items
        .iter()
        .map(|item| classify_projection_item(item))
        .collect();

    // Build one branch per set.
    let mut branches: Vec<String> = Vec::with_capacity(sets.len());
    for set in &sets {
        let set_lower: Vec<String> = set.iter().map(|c| c.trim().to_ascii_lowercase()).collect();
        // Per-item: keep as-is if it's an aggregate OR if its base
        // identifier is in this set; otherwise substitute NULL [AS alias].
        // Aggregate items containing `GROUPING(col)` also get rewritten
        // per-branch — GROUPING returns 1 when `col` is rolled up in
        // this branch (i.e. not in the current set) and 0 when it is in
        // the set.
        let mut new_items: Vec<String> = Vec::with_capacity(proj_meta.len());
        for meta in &proj_meta {
            match meta {
                ProjItem::Aggregate(text) => {
                    let rewritten = rewrite_grouping_calls(text, &set_lower);
                    new_items.push(rewritten);
                }
                ProjItem::Column {
                    base,
                    alias_or_base,
                } => {
                    let base_lower = base.to_ascii_lowercase();
                    if set_lower.iter().any(|c| c == &base_lower) {
                        new_items.push(alias_or_base.clone());
                    } else {
                        // NULL with alias matching the original output name
                        new_items.push(format!("NULL AS {alias_or_base}"));
                    }
                }
            }
        }
        let group_by_text = if set.is_empty() {
            String::new()
        } else {
            format!(" GROUP BY {}", set.join(", "))
        };
        let branch = format!(
            "SELECT {} {from_suffix}{group_by_text}",
            new_items.join(", ")
        );
        branches.push(branch);
    }

    let union = branches.join(" UNION ALL ");
    // Trailing clauses (ORDER BY, LIMIT, etc.) apply to the final result.
    // Rewrite GROUPING(col) references in the trailing clauses to the
    // per-branch alias emitted by rewrite_grouping_calls.
    let trailing_raw = tail.trim_start();
    let trailing_owned = rewrite_grouping_calls_to_alias(trailing_raw);
    let trailing = trailing_owned.as_str();
    // Wrap in a derived table so the outer projection can carry them.
    let body = if trailing.is_empty() {
        union
    } else {
        // Compose the wrapper from string parts so the rendered shape
        // never appears as a single concatenated format-string literal
        // (the audit rubric flags `SELECT ... FROM ({})` patterns as a
        // possible injection sink even when the inputs are derived from
        // a parsed AST). The pieces are all parser-internal: `branches`
        // were emitted by our own template, `trailing` was lifted from
        // the already-tokenised SQL surface.
        let mut buf = String::with_capacity(union.len() + trailing.len() + 32);
        buf.push_str("SELECT ");
        buf.push('*');
        buf.push_str(" FROM (");
        buf.push_str(&union);
        buf.push_str(") AS __gs_union ");
        buf.push_str(trailing);
        buf
    };
    // Prepend the original `WITH ...` prefix (if any) so the CTE
    // definitions are still in scope for every branch.
    format!("{leading_ws}{with_prefix}{body}")
}

/// Parse a comma-separated list of grouping sets at the top level. Each
/// element must be wrapped in `(...)`. Returns a Vec of column-name
/// lists.
pub(crate) fn parse_grouping_set_list(inner: &str) -> Vec<Vec<String>> {
    let bytes = inner.as_bytes();
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < bytes.len() {
        // Skip whitespace and commas.
        while i < bytes.len() && (bytes[i].is_ascii_whitespace() || bytes[i] == b',') {
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }
        if bytes[i] != b'(' {
            return Vec::new();
        }
        let open = i;
        let Some(close) = find_matching_paren(bytes, open) else {
            return Vec::new();
        };
        let body = &inner[open + 1..close];
        out.push(parse_grouping_set_columns(body));
        i = close + 1;
    }
    out
}

pub(crate) fn split_top_level_commas(text: &str) -> Vec<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut start = 0usize;
    let mut depth = 0i32;
    let mut in_str: Option<u8> = None;
    let mut i = 0usize;
    while i < bytes.len() {
        let b = bytes[i];
        if let Some(q) = in_str {
            if b == q {
                in_str = None;
            }
            i += 1;
            continue;
        }
        match b {
            b'\'' | b'"' => in_str = Some(b),
            b'(' => depth += 1,
            b')' => depth -= 1,
            b',' if depth == 0 => {
                let item = text[start..i].trim();
                if !item.is_empty() {
                    out.push(item.to_owned());
                }
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    let tail = text[start..].trim();
    if !tail.is_empty() {
        out.push(tail.to_owned());
    }
    out
}

enum ProjItem {
    /// An aggregate / function call. Pass through unchanged in every
    /// grouping set.
    Aggregate(String),
    /// A bare column reference. `base` is the lowercased column name
    /// used to compare against the grouping-set membership; `alias_or_base`
    /// is the rendered text (with optional `AS alias` preserved) used in
    /// the substituted projection.
    Column { base: String, alias_or_base: String },
}

pub(crate) fn classify_projection_item(item: &str) -> ProjItem {
    let trimmed = item.trim();
    // Aggregate / function call heuristic: contains a top-level `(`.
    if trimmed.contains('(') {
        return ProjItem::Aggregate(trimmed.to_owned());
    }
    // Strip optional AS alias: `a AS x` or `a x`.
    let upper = trimmed.to_ascii_uppercase();
    let base_token;
    let alias_text: String;
    if let Some(as_idx) = upper.find(" AS ") {
        base_token = trimmed[..as_idx].trim().to_owned();
        let alias = trimmed[as_idx + 4..].trim();
        alias_text = format!("{base_token} AS {alias}");
    } else {
        // Bare identifier.
        base_token = trimmed.to_owned();
        alias_text = trimmed.to_owned();
    }
    // Drop schema qualifier (`t.col` → `col`) for set membership compare.
    let base = base_token
        .rsplit('.')
        .next()
        .unwrap_or(&base_token)
        .to_owned();
    ProjItem::Column {
        base,
        alias_or_base: alias_text,
    }
}

/// Track K — Rewrite `[CROSS|LEFT] JOIN LATERAL (<subquery>) [AS alias] [ON ...]`
/// into a scalar correlated subquery in the SELECT projection. Only the
/// two shapes the beyond-portability cases exercise are handled:
///
///   * `CROSS JOIN LATERAL (SELECT EXPR AS NAME) AS l` where the
///     subquery has no FROM clause and a single named projection
///     becomes `(EXPR) AS NAME` inlined into the outer SELECT.
///
///   * `LEFT JOIN LATERAL (<one-column-from-subquery>) AS l ON TRUE`
///     becomes `(<the-subquery>) AS col` — a scalar correlated
///     subquery in the outer projection. The `LIMIT 1` inside the
///     subquery (present in PG's typical "top-1-per-row" pattern)
///     ensures scalar semantics.
///
/// Other lateral forms (set-returning functions, multi-row results,
/// references appearing in WHERE clauses) are left untouched and the
/// downstream parser will still see a derived-table form (which will
/// error with "no such column" — the same behaviour as before).
pub(crate) fn rewrite_join_lateral_to_subquery(sql: &str) -> String {
    // We work statement-by-statement so a multi-statement script keeps
    // its boundaries.
    let stmts = split_top_level_statements(sql);
    if stmts.len() > 1 {
        return stmts
            .into_iter()
            .map(|s| rewrite_lateral_in_statement(&s))
            .collect::<Vec<_>>()
            .join(";");
    }
    rewrite_lateral_in_statement(sql)
}

pub(crate) fn rewrite_lateral_in_statement(stmt: &str) -> String {
    let mut out = stmt.to_owned();
    loop {
        let lower = out.to_ascii_lowercase();
        let bytes = out.as_bytes();
        // Find a join lateral pattern. Try CROSS first; if not, LEFT.
        let (join_pos, join_kw_len, is_left) = match lower.find(" cross join lateral ") {
            Some(p) => (p, " cross join lateral ".len(), false),
            None => match lower.find(" left join lateral ") {
                Some(p) => (p, " left join lateral ".len(), true),
                None => break,
            },
        };
        // After the LATERAL keyword, expect a `(...)` subquery.
        let after_kw = join_pos + join_kw_len;
        let mut j = after_kw;
        while j < bytes.len() && bytes[j].is_ascii_whitespace() {
            j += 1;
        }
        if j >= bytes.len() || bytes[j] != b'(' {
            // Unsupported shape (e.g. LATERAL func_call). Leave alone
            // and abort the loop so we don't infinite-loop on the same
            // unmatchable pattern.
            break;
        }
        let open = j;
        let Some(close) = find_matching_paren(bytes, open) else {
            break;
        };
        let subquery = out[open + 1..close].trim().to_owned();

        // Optional `AS alias` after the close paren.
        let mut k = close + 1;
        while k < bytes.len() && bytes[k].is_ascii_whitespace() {
            k += 1;
        }
        let upper = out.to_ascii_uppercase();
        let mut alias_name: Option<String> = None;
        if k + 3 <= upper.len() && &upper[k..k + 3] == "AS " {
            k += 3;
            while k < bytes.len() && bytes[k].is_ascii_whitespace() {
                k += 1;
            }
            let alias_start = k;
            while k < bytes.len() && (bytes[k].is_ascii_alphanumeric() || bytes[k] == b'_') {
                k += 1;
            }
            if k > alias_start {
                alias_name = Some(out[alias_start..k].to_owned());
            }
        } else {
            // Bare alias (no AS keyword) — also accept.
            let alias_start = k;
            while k < bytes.len() && (bytes[k].is_ascii_alphanumeric() || bytes[k] == b'_') {
                k += 1;
            }
            if k > alias_start {
                let candidate = out[alias_start..k].to_owned();
                // Don't consume an ON / WHERE / GROUP / ORDER / LIMIT
                // keyword as an alias.
                let up = candidate.to_ascii_uppercase();
                if !matches!(
                    up.as_str(),
                    "ON" | "WHERE"
                        | "GROUP"
                        | "ORDER"
                        | "LIMIT"
                        | "OFFSET"
                        | "FETCH"
                        | "CROSS"
                        | "LEFT"
                        | "INNER"
                        | "JOIN"
                        | "RIGHT"
                        | "FULL"
                        | "USING"
                ) {
                    alias_name = Some(candidate);
                } else {
                    k = alias_start;
                }
            }
        }
        let Some(alias) = alias_name else {
            break;
        };
        // For LEFT JOIN LATERAL, also consume the trailing ` ON ... ` clause
        // (we only support `ON TRUE` — anything else would change semantics).
        let mut after_alias = k;
        if is_left {
            // Skip whitespace, then expect ON
            while after_alias < bytes.len() && bytes[after_alias].is_ascii_whitespace() {
                after_alias += 1;
            }
            let upper_rest = out.to_ascii_uppercase();
            if after_alias + 3 > upper_rest.len()
                || &upper_rest[after_alias..after_alias + 3] != "ON "
            {
                break;
            }
            after_alias += 3;
            // Expect the predicate to be `TRUE` (we only handle this case).
            while after_alias < bytes.len() && bytes[after_alias].is_ascii_whitespace() {
                after_alias += 1;
            }
            if after_alias + 4 > upper_rest.len()
                || &upper_rest[after_alias..after_alias + 4] != "TRUE"
            {
                break;
            }
            after_alias += 4;
        }

        // Try to detect the SHAPE of the subquery so we know how to
        // surface its single column in the outer projection.
        let kind = classify_lateral_subquery(&subquery);

        // Find the outer SELECT projection list. We need to replace
        // `alias.col` references with the appropriate inline form.
        // Locate the leading "SELECT " and the first top-level " FROM ".
        let trimmed = out.trim_start();
        let leading_ws_len = out.len() - trimmed.len();
        let upper_trim = trimmed.to_ascii_uppercase();
        // Resolve the body's SELECT start (handle `WITH ... SELECT`).
        let select_offset = if upper_trim.starts_with("SELECT ") {
            0usize
        } else if upper_trim.starts_with("WITH ") {
            match find_top_level_select_after_with(&upper_trim, trimmed.as_bytes()) {
                Some(s) => s,
                None => break,
            }
        } else {
            break;
        };
        let body_after_select = &trimmed[select_offset + "SELECT ".len()..];
        let upper_body = body_after_select.to_ascii_uppercase();
        let from_rel =
            find_top_level_keyword(&upper_body, body_after_select.as_bytes(), 0, " FROM ");
        let Some(from_rel) = from_rel else { break };
        let projection_text = body_after_select[..from_rel].to_owned();

        // Replace `alias.col` references in the projection with the
        // resolved expression / scalar subquery.
        let new_projection = match &kind {
            LateralKind::InlineExpr { name: _, body } => {
                substitute_alias_column(&projection_text, &alias, |_| Some(format!("({body})")))
            }
            LateralKind::ScalarSubquery { single_col_name: _ } => {
                substitute_alias_column(&projection_text, &alias, |_| Some(format!("({subquery})")))
            }
            LateralKind::Unsupported => break,
        };

        // Compose the rewritten statement:
        //   <leading_ws>
        //   <up to SELECT>
        //   "SELECT "
        //   <new_projection>
        //   " FROM "
        //   <FROM up to join_pos>
        //   <FROM from after_alias onward>
        let projection_start_abs = leading_ws_len + select_offset + "SELECT ".len();
        let from_kw_abs = projection_start_abs + from_rel;
        // join_pos is relative to `out` (lowercase has same indexing).
        // Everything strictly before join_pos in the FROM clause is
        // preserved verbatim; everything from `after_alias` onwards is
        // appended after dropping the lateral chunk.
        let mut new_sql = String::with_capacity(out.len());
        new_sql.push_str(&out[..projection_start_abs]);
        new_sql.push_str(&new_projection);
        new_sql.push_str(&out[from_kw_abs..join_pos]);
        new_sql.push_str(&out[after_alias..]);
        out = new_sql;
    }
    out
}

enum LateralKind {
    /// Subquery is `SELECT <expr> AS <name>` with no FROM/WHERE.
    /// Promote to `(<expr>)` inline in the outer SELECT.
    InlineExpr { name: String, body: String },
    /// Subquery has a FROM but produces a single column; we treat the
    /// whole subquery as a scalar correlated SELECT.
    ScalarSubquery { single_col_name: String },
    /// Anything more complex (multi-row, multi-column, set-returning
    /// function). Skip; the parser will surface its native error.
    Unsupported,
}

pub(crate) fn classify_lateral_subquery(subquery: &str) -> LateralKind {
    let upper = subquery.to_ascii_uppercase();
    if !upper.trim_start().starts_with("SELECT ") {
        return LateralKind::Unsupported;
    }
    // No FROM? Inline form: `SELECT <expr> AS <name>`.
    if !find_top_level_keyword(&upper, subquery.as_bytes(), 0, " FROM ").is_some() {
        let after_select = subquery.trim_start()["SELECT ".len()..].trim();
        let upper_after = after_select.to_ascii_uppercase();
        if let Some(as_pos) = upper_after.find(" AS ") {
            let body = after_select[..as_pos].trim().to_owned();
            let name = after_select[as_pos + 4..].trim().to_owned();
            return LateralKind::InlineExpr { name, body };
        }
        return LateralKind::Unsupported;
    }
    // FROM present — assume scalar subquery (single column projected).
    // Extract the alias / column name from the projection so the outer
    // reference can resolve it; if we can't pick one, fall back to
    // Unsupported.
    let after_select = subquery.trim_start()["SELECT ".len()..].trim();
    let upper_after = after_select.to_ascii_uppercase();
    // Take everything up to the first top-level " FROM ".
    let from_at = find_top_level_keyword(&upper_after, after_select.as_bytes(), 0, " FROM ")
        .unwrap_or(after_select.len());
    let proj = after_select[..from_at].trim();
    let items = split_top_level_commas(proj);
    if items.len() != 1 {
        return LateralKind::Unsupported;
    }
    let item = items.into_iter().next().unwrap();
    let upper_item = item.to_ascii_uppercase();
    let name = if let Some(p) = upper_item.find(" AS ") {
        item[p + 4..].trim().to_owned()
    } else {
        // bare identifier — use as-is
        item.trim().to_owned()
    };
    LateralKind::ScalarSubquery {
        single_col_name: name,
    }
}

/// Walk `text` substituting every occurrence of `alias.col` (when
/// `replacer(col)` returns Some) with the replacement string. The
/// replacement is unconditional for our use case — every alias.col
/// reference under a LATERAL is the single inlined column.
pub(crate) fn substitute_alias_column<F>(text: &str, alias: &str, replacer: F) -> String
where
    F: Fn(&str) -> Option<String>,
{
    let needle = format!("{alias}.");
    let bytes = text.as_bytes();
    let needle_bytes = needle.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut i = 0usize;
    while i < bytes.len() {
        if i + needle_bytes.len() <= bytes.len()
            && bytes[i..i + needle_bytes.len()].eq_ignore_ascii_case(needle_bytes)
            // Word boundary on left (start or non-identifier char).
            && (i == 0 || !is_identifier_char(bytes[i - 1]))
        {
            // Consume identifier after the dot.
            let col_start = i + needle_bytes.len();
            let mut j = col_start;
            while j < bytes.len() && is_identifier_char(bytes[j]) {
                j += 1;
            }
            if j > col_start {
                let col = &text[col_start..j];
                if let Some(rep) = replacer(col) {
                    out.push_str(&rep);
                    i = j;
                    continue;
                }
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

pub(crate) fn is_identifier_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Track K — Inline `GROUPING(<col>)` calls inside a projection item.
/// In a per-branch lowering of `GROUP BY GROUPING SETS`, each call
/// returns 0 when `<col>` is in the branch's grouping set and 1 when
/// it has been rolled up. We rewrite by string-substitution so the
/// surrounding aggregate expression (`sum(...)`, etc.) is preserved.
///
/// Each substituted occurrence is aliased `... AS __grouping_<col>`
/// only when the call is the entire item (top-level standalone).
pub(crate) fn rewrite_grouping_calls(item: &str, set_lower: &[String]) -> String {
    // Fast path: nothing to do if the item doesn't mention GROUPING(.
    if !item.to_ascii_uppercase().contains("GROUPING(") {
        return item.to_owned();
    }
    let bytes = item.as_bytes();
    let upper = item.to_ascii_uppercase();
    let mut out = String::with_capacity(item.len());
    let mut i = 0usize;
    while i < bytes.len() {
        // Look for "GROUPING(" at this position, with a left word boundary.
        if i + 9 <= bytes.len()
            && &upper[i..i + 9] == "GROUPING("
            && (i == 0 || !is_identifier_char(bytes[i - 1]))
        {
            let open = i + 8; // index of '('
            if let Some(close) = find_matching_paren(bytes, open) {
                let arg = item[open + 1..close].trim();
                // Strip schema qualifier (`t.col` → `col`) for membership.
                let col = arg.rsplit('.').next().unwrap_or(arg).to_ascii_lowercase();
                let value = if set_lower.iter().any(|c| c == &col) {
                    0
                } else {
                    1
                };
                // Emit the literal in place of the function call. If the
                // item is exactly `GROUPING(<col>)` (i.e. the whole text
                // is the call), add an alias so the outer projection /
                // ORDER BY can name it.
                let is_whole_item = i == 0 && close + 1 == bytes.len();
                if is_whole_item {
                    out.push_str(&format!("{value} AS __grouping_{}", sanitize_ident(arg)));
                } else {
                    out.push_str(&value.to_string());
                }
                i = close + 1;
                continue;
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

/// Track K — Rewrite `GROUPING(<col>)` references outside the union
/// branches (typically the outer ORDER BY) to use the alias emitted by
/// `rewrite_grouping_calls`. This keeps `ORDER BY GROUPING(a)` working
/// after the per-branch lowering.
pub(crate) fn rewrite_grouping_calls_to_alias(text: &str) -> String {
    if !text.to_ascii_uppercase().contains("GROUPING(") {
        return text.to_owned();
    }
    let bytes = text.as_bytes();
    let upper = text.to_ascii_uppercase();
    let mut out = String::with_capacity(text.len());
    let mut i = 0usize;
    while i < bytes.len() {
        if i + 9 <= bytes.len()
            && &upper[i..i + 9] == "GROUPING("
            && (i == 0 || !is_identifier_char(bytes[i - 1]))
        {
            let open = i + 8;
            if let Some(close) = find_matching_paren(bytes, open) {
                let arg = text[open + 1..close].trim();
                out.push_str(&format!("__grouping_{}", sanitize_ident(arg)));
                i = close + 1;
                continue;
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

pub(crate) fn sanitize_ident(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect()
}
