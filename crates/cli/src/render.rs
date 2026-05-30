use std::cmp::max;
use std::fmt::Write as _;
use std::io::Write;

use redlinedb::ValueRef;

use crate::dot::OutputMode;

#[derive(Clone, Debug)]
pub(crate) enum Cell {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
    Blob(Vec<u8>),
}

impl Cell {
    pub(crate) fn from_value_ref(value: ValueRef<'_>) -> Self {
        match value {
            ValueRef::Null => Self::Null,
            ValueRef::Integer(v) => Self::Integer(v),
            ValueRef::Real(v) => Self::Real(v),
            ValueRef::Text(v) => Self::Text(v.to_owned()),
            ValueRef::Blob(v) => Self::Blob(v.to_vec()),
        }
    }

    fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }

    fn is_text(&self) -> bool {
        matches!(self, Self::Text(_))
    }

    /// Display the cell as plain text (no quoting / escaping). NULLs are
    /// rendered as an empty string; callers may substitute their own NULL
    /// representation.
    fn text_value(&self) -> String {
        match self {
            Self::Null => String::new(),
            Self::Integer(v) => v.to_string(),
            Self::Real(v) => format_real(*v),
            Self::Text(v) => v.clone(),
            Self::Blob(v) => format_blob_text(v),
        }
    }

    fn literal_value(&self) -> String {
        match self {
            Self::Null => "NULL".to_owned(),
            Self::Integer(v) => v.to_string(),
            Self::Real(v) => format_real(*v),
            Self::Text(v) => {
                let mut out = String::with_capacity(v.len() + 2);
                out.push('\'');
                out.push_str(&v.replace('\'', "''"));
                out.push('\'');
                out
            }
            // sqlite3 renders blob literals in INSERT statements with a
            // lowercase `x'..'` prefix and lowercase hex digits.
            Self::Blob(v) => format_blob_literal(v, true),
        }
    }

    /// Render the cell as a JSON value fragment using SQLite's own encoding
    /// rules. Strings and numbers go through serde_json so the escape
    /// machinery handles control characters; blobs always serialise as a
    /// JSON string of `\u00XX` escapes (one per byte) to match the sqlite3
    /// shell's output.
    fn write_json<W: Write>(&self, out: &mut W) -> Result<(), String> {
        match self {
            Self::Null => out.write_all(b"null").map_err(|e| e.to_string()),
            Self::Integer(v) => write!(out, "{v}").map_err(|e| e.to_string()),
            Self::Real(v) => match serde_json::Number::from_f64(*v) {
                Some(n) => write!(out, "{n}").map_err(|e| e.to_string()),
                None => out.write_all(b"null").map_err(|e| e.to_string()),
            },
            Self::Text(v) => {
                let body = serde_json::to_string(v).map_err(|e| e.to_string())?;
                out.write_all(body.as_bytes()).map_err(|e| e.to_string())
            }
            Self::Blob(v) => {
                out.write_all(b"\"").map_err(|e| e.to_string())?;
                for byte in v {
                    let _ = write!(out, "\\u{:04x}", *byte as u32);
                }
                out.write_all(b"\"").map_err(|e| e.to_string())
            }
        }
    }
}

pub(crate) fn render_query<W: Write>(
    out: &mut W,
    mode: OutputMode,
    separator: &str,
    show_header: bool,
    null_value: &str,
    insert_table_name: &str,
    user_widths: &[usize],
    column_names: &[String],
    rows: &[Vec<Cell>],
) -> Result<(), String> {
    match mode {
        OutputMode::Json => render_json(out, column_names, rows),
        OutputMode::Quote => render_quote(out, column_names, rows, separator, show_header),
        OutputMode::Line => render_line(out, column_names, rows, null_value),
        OutputMode::Ascii => render_ascii(out, column_names, rows, show_header, null_value),
        OutputMode::Csv | OutputMode::List | OutputMode::Tabs => render_delimited(
            out,
            mode,
            separator,
            column_names,
            rows,
            show_header,
            null_value,
        ),
        OutputMode::Column => render_column(
            out,
            column_names,
            rows,
            show_header,
            null_value,
            user_widths,
        ),
        OutputMode::Table => render_table(
            out,
            column_names,
            rows,
            show_header,
            null_value,
            user_widths,
        ),
        OutputMode::Box => render_box(
            out,
            column_names,
            rows,
            show_header,
            null_value,
            user_widths,
        ),
        OutputMode::Markdown => render_markdown(
            out,
            column_names,
            rows,
            show_header,
            null_value,
            user_widths,
        ),
        OutputMode::Html => render_html(out, column_names, rows, show_header),
        OutputMode::Insert => render_insert(out, column_names, rows, insert_table_name),
        OutputMode::Tcl => render_tcl(out, column_names, rows, separator, show_header, null_value),
    }
}

fn render_json<W: Write>(
    out: &mut W,
    column_names: &[String],
    rows: &[Vec<Cell>],
) -> Result<(), String> {
    if column_names.is_empty() && rows.is_empty() {
        return Ok(());
    }
    // SQLite shell prints each row on its own line: `[{..},\n{..},\n{..}]`.
    // Cells are emitted via `Cell::write_json` so blob escapes match the
    // shell's `\u00XX`-per-byte convention.
    out.write_all(b"[").map_err(|err| err.to_string())?;
    for (row_idx, row) in rows.iter().enumerate() {
        if row_idx > 0 {
            out.write_all(b",\n").map_err(|err| err.to_string())?;
        }
        out.write_all(b"{").map_err(|err| err.to_string())?;
        for (col_idx, cell) in row.iter().enumerate() {
            if col_idx > 0 {
                out.write_all(b",").map_err(|err| err.to_string())?;
            }
            let default = format!("column{col_idx}");
            let key = column_names
                .get(col_idx)
                .map(String::as_str)
                .unwrap_or(&default);
            let key_json = serde_json::to_string(key).map_err(|err| err.to_string())?;
            out.write_all(key_json.as_bytes())
                .map_err(|err| err.to_string())?;
            out.write_all(b":").map_err(|err| err.to_string())?;
            cell.write_json(out)?;
        }
        out.write_all(b"}").map_err(|err| err.to_string())?;
    }
    out.write_all(b"]\n").map_err(|err| err.to_string())
}

fn render_quote<W: Write>(
    out: &mut W,
    column_names: &[String],
    rows: &[Vec<Cell>],
    _separator: &str,
    show_header: bool,
) -> Result<(), String> {
    if show_header && !column_names.is_empty() {
        let header: Vec<String> = column_names.iter().map(|name| quote_string(name)).collect();
        write_delimited_row(out, &header, OutputMode::List, ",", true)?;
    }
    for row in rows {
        let values: Vec<String> = row
            .iter()
            .map(|cell| match cell {
                // Quote mode renders blobs with lowercase `x'..'` to mirror
                // the sqlite3 shell behaviour.
                Cell::Blob(bytes) => format_blob_literal(bytes, true),
                other => other.literal_value(),
            })
            .collect();
        write_delimited_row(out, &values, OutputMode::List, ",", true)?;
    }
    Ok(())
}

fn render_line<W: Write>(
    out: &mut W,
    column_names: &[String],
    rows: &[Vec<Cell>],
    null_value: &str,
) -> Result<(), String> {
    if rows.is_empty() {
        return Ok(());
    }
    // sqlite3 right-aligns column names to the longest column-name length and
    // separates with ": ". Continuation lines (values containing newlines)
    // are indented to align with the value column.
    let name_width = column_names
        .iter()
        .map(|name| name.chars().count())
        .max()
        .unwrap_or(0);
    let prefix_len = name_width + 2; // " : "
    let indent = " ".repeat(prefix_len);
    for (row_index, row) in rows.iter().enumerate() {
        for (index, value) in row.iter().enumerate() {
            let name = column_names
                .get(index)
                .map(String::as_str)
                .unwrap_or("column");
            let rendered = if value.is_null() {
                null_value.to_owned()
            } else {
                value.text_value()
            };
            let padded_name = format!("{name:>width$}", width = name_width);
            let mut first = true;
            for part in rendered.split('\n') {
                if first {
                    writeln!(out, "{padded_name}: {part}").map_err(|err| err.to_string())?;
                    first = false;
                } else {
                    writeln!(out, "{indent}{part}").map_err(|err| err.to_string())?;
                }
            }
        }
        if row_index + 1 != rows.len() {
            writeln!(out).map_err(|err| err.to_string())?;
        }
    }
    Ok(())
}

fn render_ascii<W: Write>(
    out: &mut W,
    column_names: &[String],
    rows: &[Vec<Cell>],
    show_header: bool,
    null_value: &str,
) -> Result<(), String> {
    if show_header && !column_names.is_empty() {
        write_joined(
            out,
            column_names.iter().map(|name| name.as_str()),
            "\x1f",
            false,
        )?;
        out.write_all(b"\x1e").map_err(|err| err.to_string())?;
    }
    for row in rows {
        let rendered = row.iter().map(|cell| {
            if cell.is_null() {
                null_value.to_owned()
            } else {
                cell.text_value()
            }
        });
        write_joined(out, rendered, "\x1f", false)?;
        out.write_all(b"\x1e").map_err(|err| err.to_string())?;
    }
    Ok(())
}

fn render_delimited<W: Write>(
    out: &mut W,
    mode: OutputMode,
    separator: &str,
    column_names: &[String],
    rows: &[Vec<Cell>],
    show_header: bool,
    null_value: &str,
) -> Result<(), String> {
    // sqlite3 terminates every CSV row with CRLF (per RFC 4180); the other
    // delimited modes use plain LF.
    let row_terminator: &[u8] = if matches!(mode, OutputMode::Csv) {
        b"\r\n"
    } else {
        b"\n"
    };
    let mut wrote_anything = false;
    if show_header && !column_names.is_empty() {
        let header: Vec<String> = column_names.iter().cloned().collect();
        write_delimited_row(out, &header, mode, separator, false)?;
        wrote_anything = true;
    }
    for row in rows {
        if wrote_anything {
            out.write_all(row_terminator)
                .map_err(|err| err.to_string())?;
        }
        let values: Vec<String> = row
            .iter()
            .map(|cell| {
                if cell.is_null() {
                    null_value.to_owned()
                } else {
                    cell.text_value()
                }
            })
            .collect();
        write_delimited_row(out, &values, mode, separator, false)?;
        wrote_anything = true;
    }
    if wrote_anything {
        out.write_all(row_terminator)
            .map_err(|err| err.to_string())?;
    }
    Ok(())
}

pub(crate) fn is_streaming_delimited_mode(mode: OutputMode) -> bool {
    matches!(mode, OutputMode::Csv | OutputMode::List | OutputMode::Tabs)
}

pub(crate) fn write_stream_delimited_value<W: Write>(
    out: &mut W,
    mode: OutputMode,
    separator: &str,
    null_value: &str,
    escape_symbol: bool,
    value: ValueRef<'_>,
) -> Result<(), String> {
    match mode {
        OutputMode::Csv => write_csv_value_ref(out, value, separator, null_value),
        OutputMode::Tabs => write_tabs_value_ref(out, value, null_value, escape_symbol),
        OutputMode::List => write_text_value_ref(out, value, null_value, escape_symbol),
        _ => Err("streaming renderer only supports delimited modes".to_owned()),
    }
}

// ---------------------------------------------------------------------------
// Column / box / table / markdown — shared layout helpers
// ---------------------------------------------------------------------------

/// Compute per-column widths sized to the widest cell in each column. Headers
/// also count for the width because the header row must fit.
///
/// `user_widths` overrides the computed width for the corresponding columns.
/// A user width of 0 keeps the auto-sized value, matching sqlite3's `.width
/// 0` "use default" sentinel.
fn column_widths(
    column_names: &[String],
    rows: &[Vec<Cell>],
    null_value: &str,
    user_widths: &[usize],
) -> Vec<usize> {
    let mut widths = vec![0usize; column_names.len()];
    for (index, name) in column_names.iter().enumerate() {
        widths[index] = max(widths[index], name.chars().count());
    }
    for row in rows {
        if widths.len() < row.len() {
            widths.resize(row.len(), 0);
        }
        for (index, cell) in row.iter().enumerate() {
            let text = if cell.is_null() {
                null_value.to_owned()
            } else {
                cell.text_value()
            };
            widths[index] = max(widths[index], text.chars().count());
        }
    }
    for (index, override_width) in user_widths.iter().enumerate() {
        if index < widths.len() && *override_width > 0 {
            widths[index] = *override_width;
        }
    }
    widths
}

/// Centre `value` within a field of `width` columns. If the value is wider
/// than the field, the value is returned as-is.
fn center_pad(value: &str, width: usize) -> String {
    let value_len = value.chars().count();
    if value_len >= width {
        return value.to_owned();
    }
    let pad = width - value_len;
    let left = pad / 2;
    let right = pad - left;
    let mut out = String::with_capacity(width);
    for _ in 0..left {
        out.push(' ');
    }
    out.push_str(value);
    for _ in 0..right {
        out.push(' ');
    }
    out
}

/// Left-align `value` within `width` chars.
fn left_pad(value: &str, width: usize) -> String {
    let value_len = value.chars().count();
    if value_len >= width {
        return value.to_owned();
    }
    let mut out = String::with_capacity(width);
    out.push_str(value);
    for _ in 0..(width - value_len) {
        out.push(' ');
    }
    out
}

/// Right-align `value` within `width` chars.
fn right_pad(value: &str, width: usize) -> String {
    let value_len = value.chars().count();
    if value_len >= width {
        return value.to_owned();
    }
    let mut out = String::with_capacity(width);
    for _ in 0..(width - value_len) {
        out.push(' ');
    }
    out.push_str(value);
    out
}

/// Render a cell into a fixed-width slot using SQLite's column-mode alignment
/// rules: TEXT → left-aligned, NULL → left-aligned, numeric → right-aligned.
fn align_cell(cell: &Cell, width: usize, null_value: &str) -> String {
    let rendered = if cell.is_null() {
        null_value.to_owned()
    } else {
        cell.text_value()
    };
    if cell.is_text() || cell.is_null() {
        left_pad(&rendered, width)
    } else {
        right_pad(&rendered, width)
    }
}

fn render_column<W: Write>(
    out: &mut W,
    column_names: &[String],
    rows: &[Vec<Cell>],
    show_header: bool,
    null_value: &str,
    user_widths: &[usize],
) -> Result<(), String> {
    let widths = column_widths(column_names, rows, null_value, user_widths);
    if widths.is_empty() {
        return Ok(());
    }
    if show_header && !column_names.is_empty() {
        let cells: Vec<String> = column_names
            .iter()
            .enumerate()
            .map(|(idx, name)| center_pad(name, widths[idx]))
            .collect();
        write_padded_line(out, &cells)?;
        let separators: Vec<String> = widths.iter().map(|width| "-".repeat(*width)).collect();
        write_padded_line(out, &separators)?;
    }
    // sqlite3 wraps values longer than the user-specified width across
    // multiple physical lines. We only enable wrap when the caller passed
    // a `.width` override for at least one column (the auto-sized path is
    // sized to fit every value, so no wrap is needed). When wrapping is
    // active sqlite3 also inserts a blank separator line between rows.
    let needs_wrap = !user_widths.is_empty()
        && rows
            .iter()
            .any(|row| row_needs_wrap(row, &widths, null_value));
    let mut wrote_wrapped_row = false;
    for row in rows {
        let chunks_per_col: Vec<Vec<String>> = (0..widths.len())
            .map(|idx| {
                let blank = Cell::Null;
                let cell = row.get(idx).unwrap_or(&blank);
                cell_chunks(cell, widths[idx], null_value)
            })
            .collect();
        let max_lines = chunks_per_col.iter().map(Vec::len).max().unwrap_or(1);
        if needs_wrap && wrote_wrapped_row {
            writeln!(out).map_err(|err| err.to_string())?;
        }
        for line_idx in 0..max_lines {
            let cells: Vec<String> = (0..widths.len())
                .map(|idx| {
                    chunks_per_col[idx]
                        .get(line_idx)
                        .cloned()
                        .unwrap_or_else(|| " ".repeat(widths[idx]))
                })
                .collect();
            write_padded_line(out, &cells)?;
        }
        wrote_wrapped_row = needs_wrap;
    }
    Ok(())
}

fn row_needs_wrap(row: &[Cell], widths: &[usize], null_value: &str) -> bool {
    for (idx, cell) in row.iter().enumerate() {
        let width = widths.get(idx).copied().unwrap_or(0);
        let len = if cell.is_null() {
            null_value.chars().count()
        } else {
            cell.text_value().chars().count()
        };
        if width > 0 && len > width {
            return true;
        }
    }
    false
}

/// Slice a cell into width-sized chunks, padding each chunk per the cell's
/// type-driven alignment.
fn cell_chunks(cell: &Cell, width: usize, null_value: &str) -> Vec<String> {
    let rendered = if cell.is_null() {
        null_value.to_owned()
    } else {
        cell.text_value()
    };
    if width == 0 || rendered.chars().count() <= width {
        return vec![align_cell(cell, width, null_value)];
    }
    let right_align = !(cell.is_text() || cell.is_null());
    let mut chunks: Vec<String> = Vec::new();
    let chars: Vec<char> = rendered.chars().collect();
    let mut start = 0;
    while start < chars.len() {
        let end = (start + width).min(chars.len());
        let slice: String = chars[start..end].iter().collect();
        let padded = if right_align {
            right_pad(&slice, width)
        } else {
            left_pad(&slice, width)
        };
        chunks.push(padded);
        start = end;
    }
    chunks
}

/// Write a row of pre-padded cells joined by two spaces, trimming trailing
/// whitespace from the final cell (matches sqlite3's column-mode output).
fn write_padded_line<W: Write>(out: &mut W, cells: &[String]) -> Result<(), String> {
    let mut buf = String::new();
    for (idx, cell) in cells.iter().enumerate() {
        if idx > 0 {
            buf.push_str("  ");
        }
        buf.push_str(cell);
    }
    // Trim only trailing whitespace from the line (last column's right-pad).
    let trimmed = buf.trim_end_matches(' ');
    writeln!(out, "{trimmed}").map_err(|err| err.to_string())
}

fn render_table<W: Write>(
    out: &mut W,
    column_names: &[String],
    rows: &[Vec<Cell>],
    show_header: bool,
    null_value: &str,
    user_widths: &[usize],
) -> Result<(), String> {
    if rows.is_empty() && !show_header {
        return Ok(());
    }
    let widths = column_widths(column_names, rows, null_value, user_widths);
    if widths.is_empty() {
        return Ok(());
    }
    write_ascii_border(out, &widths)?;
    if show_header {
        let cells: Vec<String> = column_names
            .iter()
            .enumerate()
            .map(|(idx, name)| center_pad(name, widths[idx]))
            .collect();
        write_table_row_ascii(out, &cells)?;
        write_ascii_border(out, &widths)?;
    }
    for row in rows {
        let cells: Vec<String> = (0..widths.len())
            .map(|idx| {
                let blank = Cell::Null;
                let cell = row.get(idx).unwrap_or(&blank);
                align_cell(cell, widths[idx], null_value)
            })
            .collect();
        write_table_row_ascii(out, &cells)?;
    }
    write_ascii_border(out, &widths)?;
    Ok(())
}

fn write_ascii_border<W: Write>(out: &mut W, widths: &[usize]) -> Result<(), String> {
    let mut buf = String::from("+");
    for width in widths {
        for _ in 0..(*width + 2) {
            buf.push('-');
        }
        buf.push('+');
    }
    writeln!(out, "{buf}").map_err(|err| err.to_string())
}

fn write_table_row_ascii<W: Write>(out: &mut W, cells: &[String]) -> Result<(), String> {
    let mut buf = String::from("|");
    for cell in cells {
        buf.push(' ');
        buf.push_str(cell);
        buf.push(' ');
        buf.push('|');
    }
    writeln!(out, "{buf}").map_err(|err| err.to_string())
}

fn render_box<W: Write>(
    out: &mut W,
    column_names: &[String],
    rows: &[Vec<Cell>],
    show_header: bool,
    null_value: &str,
    user_widths: &[usize],
) -> Result<(), String> {
    if rows.is_empty() && !show_header {
        return Ok(());
    }
    let widths = column_widths(column_names, rows, null_value, user_widths);
    if widths.is_empty() {
        return Ok(());
    }
    write_box_border(out, &widths, '\u{256d}', '\u{2500}', '\u{252c}', '\u{256e}')?;
    if show_header {
        let cells: Vec<String> = column_names
            .iter()
            .enumerate()
            .map(|(idx, name)| center_pad(name, widths[idx]))
            .collect();
        write_box_row(out, &cells)?;
        // Bottom border for header uses heavy-line junctions (╞ ═ ╪ ╡).
        write_box_border(out, &widths, '\u{255e}', '\u{2550}', '\u{256a}', '\u{2561}')?;
    }
    for row in rows {
        let cells: Vec<String> = (0..widths.len())
            .map(|idx| {
                let blank = Cell::Null;
                let cell = row.get(idx).unwrap_or(&blank);
                align_cell(cell, widths[idx], null_value)
            })
            .collect();
        write_box_row(out, &cells)?;
    }
    write_box_border(out, &widths, '\u{2570}', '\u{2500}', '\u{2534}', '\u{256f}')?;
    Ok(())
}

fn write_box_border<W: Write>(
    out: &mut W,
    widths: &[usize],
    left: char,
    fill: char,
    middle: char,
    right: char,
) -> Result<(), String> {
    let mut buf = String::new();
    buf.push(left);
    for (idx, width) in widths.iter().enumerate() {
        if idx > 0 {
            buf.push(middle);
        }
        for _ in 0..(*width + 2) {
            buf.push(fill);
        }
    }
    buf.push(right);
    writeln!(out, "{buf}").map_err(|err| err.to_string())
}

fn write_box_row<W: Write>(out: &mut W, cells: &[String]) -> Result<(), String> {
    let mut buf = String::new();
    buf.push('\u{2502}');
    for cell in cells {
        buf.push(' ');
        buf.push_str(cell);
        buf.push(' ');
        buf.push('\u{2502}');
    }
    writeln!(out, "{buf}").map_err(|err| err.to_string())
}

fn render_markdown<W: Write>(
    out: &mut W,
    column_names: &[String],
    rows: &[Vec<Cell>],
    show_header: bool,
    null_value: &str,
    user_widths: &[usize],
) -> Result<(), String> {
    let widths = column_widths(column_names, rows, null_value, user_widths);
    if widths.is_empty() {
        return Ok(());
    }
    if show_header && !column_names.is_empty() {
        let cells: Vec<String> = column_names
            .iter()
            .enumerate()
            .map(|(idx, name)| center_pad(name, widths[idx]))
            .collect();
        write_markdown_row(out, &cells, ' ')?;
        // sqlite3 emits the separator with dashes filling the same width as
        // the cell padding (width + 2 for the surrounding spaces).
        let separators: Vec<String> = widths
            .iter()
            .map(|width| "-".repeat((*width + 2).max(3)))
            .collect();
        write_markdown_separator(out, &separators)?;
    }
    for row in rows {
        let cells: Vec<String> = (0..widths.len())
            .map(|idx| {
                let blank = Cell::Null;
                let cell = row.get(idx).unwrap_or(&blank);
                align_cell(cell, widths[idx], null_value)
            })
            .collect();
        write_markdown_row(out, &cells, ' ')?;
    }
    Ok(())
}

fn write_markdown_row<W: Write>(out: &mut W, cells: &[String], pad: char) -> Result<(), String> {
    let mut buf = String::from("|");
    for cell in cells {
        buf.push(pad);
        buf.push_str(cell);
        buf.push(pad);
        buf.push('|');
    }
    writeln!(out, "{buf}").map_err(|err| err.to_string())
}

fn write_markdown_separator<W: Write>(out: &mut W, dashes: &[String]) -> Result<(), String> {
    let mut buf = String::from("|");
    for d in dashes {
        buf.push_str(d);
        buf.push('|');
    }
    writeln!(out, "{buf}").map_err(|err| err.to_string())
}

fn render_html<W: Write>(
    out: &mut W,
    column_names: &[String],
    rows: &[Vec<Cell>],
    show_header: bool,
) -> Result<(), String> {
    if show_header && !column_names.is_empty() {
        writeln!(out, "<TR>").map_err(|err| err.to_string())?;
        for name in column_names {
            writeln!(out, "<TH>{}", escape_html(name)).map_err(|err| err.to_string())?;
        }
        writeln!(out, "</TR>").map_err(|err| err.to_string())?;
    }
    for row in rows {
        writeln!(out, "<TR>").map_err(|err| err.to_string())?;
        for cell in row {
            let value = match cell {
                Cell::Null => "null".to_owned(),
                _ => cell.text_value(),
            };
            writeln!(out, "<TD>{}", escape_html(&value)).map_err(|err| err.to_string())?;
        }
        writeln!(out, "</TR>").map_err(|err| err.to_string())?;
    }
    Ok(())
}

fn render_insert<W: Write>(
    out: &mut W,
    _column_names: &[String],
    rows: &[Vec<Cell>],
    table_name: &str,
) -> Result<(), String> {
    for row in rows {
        let values: Vec<String> = row.iter().map(Cell::literal_value).collect();
        writeln!(
            out,
            "INSERT INTO {table_name} VALUES({});",
            values.join(",")
        )
        .map_err(|err| err.to_string())?;
    }
    Ok(())
}

fn render_tcl<W: Write>(
    out: &mut W,
    column_names: &[String],
    rows: &[Vec<Cell>],
    separator: &str,
    show_header: bool,
    _null_value: &str,
) -> Result<(), String> {
    let sep = if separator.is_empty() { " " } else { separator };
    if show_header && !column_names.is_empty() {
        let cells: Vec<String> = column_names.iter().map(|n| tcl_quote(n)).collect();
        writeln!(out, "{}", cells.join(sep)).map_err(|err| err.to_string())?;
    }
    for row in rows {
        let cells: Vec<String> = row.iter().map(tcl_cell).collect();
        writeln!(out, "{}", cells.join(sep)).map_err(|err| err.to_string())?;
    }
    Ok(())
}

fn tcl_cell(cell: &Cell) -> String {
    match cell {
        Cell::Null => "\"\"".to_owned(),
        Cell::Integer(v) => v.to_string(),
        Cell::Real(v) => format_real(*v),
        Cell::Text(v) => tcl_quote(v),
        Cell::Blob(v) => {
            let mut s = String::from("\"");
            for byte in v {
                let _ = write!(&mut s, "\\{byte:03o}");
            }
            s.push('"');
            s
        }
    }
}

fn tcl_quote(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other if (other as u32) < 0x20 => {
                let _ = write!(&mut out, "\\{:03o}", other as u32);
            }
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

pub(crate) fn write_delimited_row<W: Write, I, S>(
    out: &mut W,
    cells: I,
    mode: OutputMode,
    separator: &str,
    append_newline: bool,
) -> Result<(), String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut first = true;
    for cell in cells {
        if !first {
            out.write_all(separator.as_bytes())
                .map_err(|err| err.to_string())?;
        }
        first = false;
        let text = cell.as_ref();
        match mode {
            OutputMode::Csv => write_csv_cell(out, text, separator)?,
            OutputMode::Tabs => write_tabs_cell(out, text)?,
            _ => out
                .write_all(text.as_bytes())
                .map_err(|err| err.to_string())?,
        }
    }
    if append_newline {
        writeln!(out).map_err(|err| err.to_string())?;
    }
    Ok(())
}

fn write_csv_cell<W: Write>(out: &mut W, text: &str, separator: &str) -> Result<(), String> {
    let needs_quotes = csv_needs_quotes(text, separator);
    if needs_quotes {
        let escaped = text.replace('"', "\"\"");
        write!(out, "\"{escaped}\"").map_err(|err| err.to_string())
    } else {
        out.write_all(text.as_bytes())
            .map_err(|err| err.to_string())
    }
}

fn csv_needs_quotes(text: &str, separator: &str) -> bool {
    if text.is_empty() {
        return false;
    }
    if text.contains(separator) || text.contains(',') {
        return true;
    }
    // Matches sqlite3's behaviour: any embedded quote, whitespace, or
    // apostrophe forces quoting so the value round-trips through CSV.
    text.chars().any(|ch| {
        matches!(ch, '"' | '\'' | '\n' | '\r' | '\t')
            || (ch.is_whitespace() && ch != ' ')
            || ch == ' '
    }) || text.starts_with(' ')
        || text.ends_with(' ')
}

fn write_tabs_cell<W: Write>(out: &mut W, text: &str) -> Result<(), String> {
    // SQLite shell quotes tab-delimited text values that contain a quote or
    // apostrophe with the same rules as CSV (minus the tab itself, which is
    // implicit in `tabs` mode).
    let needs_quotes =
        text.contains('\'') || text.contains('"') || text.contains('\n') || text.contains('\r');
    if needs_quotes {
        let escaped = text.replace('"', "\"\"");
        write!(out, "\"{escaped}\"").map_err(|err| err.to_string())
    } else {
        out.write_all(text.as_bytes())
            .map_err(|err| err.to_string())
    }
}

fn write_csv_value_ref<W: Write>(
    out: &mut W,
    value: ValueRef<'_>,
    separator: &str,
    null_value: &str,
) -> Result<(), String> {
    match value {
        ValueRef::Null => write_csv_cell(out, null_value, separator),
        ValueRef::Integer(v) => write!(out, "{v}").map_err(|err| err.to_string()),
        ValueRef::Real(v) => write!(out, "{}", format_real(v)).map_err(|err| err.to_string()),
        ValueRef::Text(v) => write_csv_cell(out, v, separator),
        ValueRef::Blob(v) => write_csv_cell(out, &format_blob_text(v), separator),
    }
}

fn write_tabs_value_ref<W: Write>(
    out: &mut W,
    value: ValueRef<'_>,
    null_value: &str,
    escape_symbol: bool,
) -> Result<(), String> {
    match value {
        ValueRef::Null => out
            .write_all(null_value.as_bytes())
            .map_err(|err| err.to_string()),
        ValueRef::Integer(v) => write!(out, "{v}").map_err(|err| err.to_string()),
        ValueRef::Real(v) => write!(out, "{}", format_real(v)).map_err(|err| err.to_string()),
        ValueRef::Text(v) => {
            if escape_symbol {
                out.write_all(escape_symbolic(v).as_bytes())
                    .map_err(|err| err.to_string())
            } else {
                write_tabs_cell(out, v)
            }
        }
        ValueRef::Blob(v) => write_tabs_cell(out, &format_blob_text(v)),
    }
}

fn write_text_value_ref<W: Write>(
    out: &mut W,
    value: ValueRef<'_>,
    null_value: &str,
    escape_symbol: bool,
) -> Result<(), String> {
    match value {
        ValueRef::Null => out
            .write_all(null_value.as_bytes())
            .map_err(|err| err.to_string()),
        ValueRef::Integer(v) => {
            // Phase 2.4: itoa is roughly 5-10x faster than Display
            // formatting for i64 and writes directly to a stack buffer
            // without any heap allocation.
            let mut buf = itoa::Buffer::new();
            out.write_all(buf.format(v).as_bytes())
                .map_err(|err| err.to_string())
        }
        ValueRef::Real(v) => write!(out, "{}", format_real(v)).map_err(|err| err.to_string()),
        ValueRef::Text(v) => {
            if escape_symbol {
                out.write_all(escape_symbolic(v).as_bytes())
                    .map_err(|err| err.to_string())
            } else {
                out.write_all(v.as_bytes()).map_err(|err| err.to_string())
            }
        }
        ValueRef::Blob(v) => out
            .write_all(format_blob_text(v).as_bytes())
            .map_err(|err| err.to_string()),
    }
}

fn escape_symbolic(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '\n' => out.push_str(r"\n"),
            '\r' => out.push_str(r"\r"),
            '\t' => out.push_str(r"\t"),
            '\\' => out.push_str(r"\\"),
            other => out.push(other),
        }
    }
    out
}

fn format_real(value: f64) -> String {
    // Delegate to the SQL crate's SQLite-compatible %!.17g formatter so
    // CLI REAL output matches sqlite3 reference (closes the
    // SQL_TYPE_AFFINITY REAL_BIG / INT_BIG cases per Track B's note).
    redlinedb::format_real_sqlite(value)
}

fn format_blob_text(bytes: &[u8]) -> String {
    if bytes.iter().all(|byte| *byte == 0) {
        return String::new();
    }
    let text = String::from_utf8_lossy(bytes);
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '\u{0000}' => out.push_str("^@"),
            '\u{0001}'..='\u{001A}' => {
                out.push('^');
                out.push((b'@' + ch as u8) as char);
            }
            '\u{007F}' => out.push_str("^?"),
            other => out.push(other),
        }
    }
    out
}

fn format_blob_literal(bytes: &[u8], lowercase: bool) -> String {
    let mut out = String::with_capacity(bytes.len() * 2 + 3);
    if lowercase {
        out.push_str("x'");
        for byte in bytes {
            let _ = write!(&mut out, "{byte:02x}");
        }
    } else {
        out.push_str("X'");
        for byte in bytes {
            let _ = write!(&mut out, "{byte:02X}");
        }
    }
    out.push('\'');
    out
}

fn write_joined<W: Write, I, S>(
    out: &mut W,
    cells: I,
    separator: &str,
    newline: bool,
) -> Result<(), String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut first = true;
    for cell in cells {
        if !first {
            out.write_all(separator.as_bytes())
                .map_err(|err| err.to_string())?;
        }
        first = false;
        out.write_all(cell.as_ref().as_bytes())
            .map_err(|err| err.to_string())?;
    }
    if newline {
        writeln!(out).map_err(|err| err.to_string())?;
    }
    Ok(())
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn quote_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('\'');
    out.push_str(&value.replace('\'', "''"));
    out.push('\'');
    out
}
