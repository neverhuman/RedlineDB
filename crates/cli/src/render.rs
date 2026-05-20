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

    fn text_value(&self) -> String {
        match self {
            Self::Null => String::new(),
            Self::Integer(v) => v.to_string(),
            Self::Real(v) => v.to_string(),
            Self::Text(v) => v.clone(),
            Self::Blob(v) => format!("<blob:{}>", v.len()),
        }
    }

    fn literal_value(&self) -> String {
        match self {
            Self::Null => "NULL".to_owned(),
            Self::Integer(v) => v.to_string(),
            Self::Real(v) => v.to_string(),
            Self::Text(v) => {
                let mut out = String::with_capacity(v.len() + 2);
                out.push('\'');
                out.push_str(&v.replace('\'', "''"));
                out.push('\'');
                out
            }
            Self::Blob(v) => {
                let mut out = String::from("X'");
                for byte in v {
                    let _ = write!(&mut out, "{byte:02X}");
                }
                out.push('\'');
                out
            }
        }
    }

    fn json_value(&self) -> serde_json::Value {
        match self {
            Self::Null => serde_json::Value::Null,
            Self::Integer(v) => serde_json::Value::from(*v),
            Self::Real(v) => serde_json::Number::from_f64(*v)
                .map(serde_json::Value::Number)
                .unwrap_or(serde_json::Value::Null),
            Self::Text(v) => serde_json::Value::String(v.clone()),
            Self::Blob(v) => serde_json::Value::Array(
                v.iter()
                    .copied()
                    .map(|byte| serde_json::Value::from(byte))
                    .collect(),
            ),
        }
    }
}

pub(crate) fn render_query<W: Write>(
    out: &mut W,
    mode: OutputMode,
    separator: &str,
    show_header: bool,
    null_value: &str,
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
        OutputMode::Column => render_column(out, column_names, rows, show_header, null_value),
        OutputMode::Table => render_table(out, column_names, rows, show_header, null_value),
        OutputMode::Markdown => render_markdown(out, column_names, rows, show_header, null_value),
        OutputMode::Html => render_html(out, column_names, rows, show_header, null_value),
        OutputMode::Insert => Err("Error: insert mode is not supported".to_owned()),
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
    let mut rendered = Vec::with_capacity(rows.len());
    for row in rows {
        let mut obj = serde_json::Map::with_capacity(column_names.len());
        for (index, value) in row.iter().enumerate() {
            let key = column_names
                .get(index)
                .cloned()
                .unwrap_or_else(|| format!("column{index}"));
            obj.insert(key, value.json_value());
        }
        rendered.push(serde_json::Value::Object(obj));
    }
    writeln!(
        out,
        "{}",
        serde_json::to_string(&rendered).map_err(|err| err.to_string())?
    )
    .map_err(|err| err.to_string())
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
        let values: Vec<String> = row.iter().map(Cell::literal_value).collect();
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
            writeln!(out, "{name} = {rendered}").map_err(|err| err.to_string())?;
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
    let mut wrote_anything = false;
    if show_header && !column_names.is_empty() {
        let header: Vec<String> = column_names.iter().cloned().collect();
        write_delimited_row(out, &header, mode, separator, false)?;
        wrote_anything = true;
    }
    for row in rows {
        if wrote_anything {
            writeln!(out).map_err(|err| err.to_string())?;
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
        writeln!(out).map_err(|err| err.to_string())?;
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
    value: ValueRef<'_>,
) -> Result<(), String> {
    match mode {
        OutputMode::Csv => write_csv_value_ref(out, value, separator, null_value),
        OutputMode::List | OutputMode::Tabs => write_text_value_ref(out, value, null_value),
        _ => Err("streaming renderer only supports delimited modes".to_owned()),
    }
}

fn render_column<W: Write>(
    out: &mut W,
    column_names: &[String],
    rows: &[Vec<Cell>],
    show_header: bool,
    null_value: &str,
) -> Result<(), String> {
    let widths = column_widths(column_names, rows, null_value);
    if show_header && !column_names.is_empty() {
        write_padded_row(out, column_names.iter().map(String::as_str), &widths)?;
        write_separator(out, &widths, '-')?;
    }
    for row in rows {
        let rendered = row.iter().map(|cell| {
            if cell.is_null() {
                null_value.to_owned()
            } else {
                cell.text_value()
            }
        });
        write_padded_row(out, rendered, &widths)?;
    }
    Ok(())
}

fn render_table<W: Write>(
    out: &mut W,
    column_names: &[String],
    rows: &[Vec<Cell>],
    show_header: bool,
    null_value: &str,
) -> Result<(), String> {
    if rows.is_empty() && !show_header {
        return Ok(());
    }
    let widths = column_widths(column_names, rows, null_value);
    if widths.is_empty() {
        return Ok(());
    }
    write_border(out, &widths, '+', '-')?;
    if show_header {
        write_box_row(out, column_names.iter().map(String::as_str), &widths, '|')?;
        write_border(out, &widths, '+', '=')?;
    }
    for row in rows {
        let rendered = row.iter().map(|cell| {
            if cell.is_null() {
                null_value.to_owned()
            } else {
                cell.text_value()
            }
        });
        write_box_row(out, rendered, &widths, '|')?;
        write_border(out, &widths, '+', '-')?;
    }
    Ok(())
}

fn render_markdown<W: Write>(
    out: &mut W,
    column_names: &[String],
    rows: &[Vec<Cell>],
    show_header: bool,
    null_value: &str,
) -> Result<(), String> {
    let widths = column_widths(column_names, rows, null_value);
    if widths.is_empty() {
        return Ok(());
    }
    if show_header {
        write_markdown_row(out, column_names.iter().map(String::as_str), &widths)?;
        let separators = widths.iter().map(|width| "-".repeat((*width).max(3)));
        write_markdown_row(out, separators, &widths)?;
    }
    for row in rows {
        let rendered = row.iter().map(|cell| {
            if cell.is_null() {
                null_value.to_owned()
            } else {
                cell.text_value()
            }
        });
        write_markdown_row(out, rendered, &widths)?;
    }
    Ok(())
}

fn render_html<W: Write>(
    out: &mut W,
    column_names: &[String],
    rows: &[Vec<Cell>],
    show_header: bool,
    null_value: &str,
) -> Result<(), String> {
    if rows.is_empty() && !show_header {
        return Ok(());
    }
    writeln!(out, "<TABLE>").map_err(|err| err.to_string())?;
    if show_header && !column_names.is_empty() {
        write!(out, "<TR>").map_err(|err| err.to_string())?;
        for name in column_names {
            write!(out, "<TH>{}</TH>", escape_html(name)).map_err(|err| err.to_string())?;
        }
        writeln!(out, "</TR>").map_err(|err| err.to_string())?;
    }
    for row in rows {
        write!(out, "<TR>").map_err(|err| err.to_string())?;
        for cell in row {
            let value = if cell.is_null() {
                null_value.to_owned()
            } else {
                cell.text_value()
            };
            write!(out, "<TD>{}</TD>", escape_html(&value)).map_err(|err| err.to_string())?;
        }
        writeln!(out, "</TR>").map_err(|err| err.to_string())?;
    }
    writeln!(out, "</TABLE>").map_err(|err| err.to_string())
}

fn column_widths(column_names: &[String], rows: &[Vec<Cell>], null_value: &str) -> Vec<usize> {
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
    widths
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
        if mode == OutputMode::Csv {
            write_csv_cell(out, text, separator)?;
        } else {
            out.write_all(text.as_bytes())
                .map_err(|err| err.to_string())?;
        }
    }
    if append_newline {
        writeln!(out).map_err(|err| err.to_string())?;
    }
    Ok(())
}

fn write_csv_cell<W: Write>(out: &mut W, text: &str, separator: &str) -> Result<(), String> {
    let needs_quotes =
        text.contains(separator) || text.contains(',') || text.contains('"') || text.contains('\n');
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
        ValueRef::Real(v) => write!(out, "{v}").map_err(|err| err.to_string()),
        ValueRef::Text(v) => write_csv_cell(out, v, separator),
        ValueRef::Blob(v) => write_csv_cell(out, &format!("<blob:{}>", v.len()), separator),
    }
}

fn write_text_value_ref<W: Write>(
    out: &mut W,
    value: ValueRef<'_>,
    null_value: &str,
) -> Result<(), String> {
    match value {
        ValueRef::Null => out
            .write_all(null_value.as_bytes())
            .map_err(|err| err.to_string()),
        ValueRef::Integer(v) => write!(out, "{v}").map_err(|err| err.to_string()),
        ValueRef::Real(v) => write!(out, "{v}").map_err(|err| err.to_string()),
        ValueRef::Text(v) => out.write_all(v.as_bytes()).map_err(|err| err.to_string()),
        ValueRef::Blob(v) => write!(out, "<blob:{}>", v.len()).map_err(|err| err.to_string()),
    }
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

fn write_padded_row<W: Write, I, S>(out: &mut W, cells: I, widths: &[usize]) -> Result<(), String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut first = true;
    for (index, cell) in cells.into_iter().enumerate() {
        if !first {
            write!(out, "  ").map_err(|err| err.to_string())?;
        }
        first = false;
        let width = widths.get(index).copied().unwrap_or(0);
        write!(out, "{:<width$}", cell.as_ref(), width = width).map_err(|err| err.to_string())?;
    }
    writeln!(out).map_err(|err| err.to_string())
}

fn write_separator<W: Write>(out: &mut W, widths: &[usize], fill: char) -> Result<(), String> {
    let mut first = true;
    for width in widths {
        if !first {
            write!(out, "  ").map_err(|err| err.to_string())?;
        }
        first = false;
        write!(out, "{fill:->width$}", width = *width).map_err(|err| err.to_string())?;
    }
    writeln!(out).map_err(|err| err.to_string())
}

fn write_border<W: Write>(
    out: &mut W,
    widths: &[usize],
    edge: char,
    fill: char,
) -> Result<(), String> {
    write!(out, "{edge}").map_err(|err| err.to_string())?;
    for width in widths {
        for _ in 0..(*width + 2) {
            write!(out, "{fill}").map_err(|err| err.to_string())?;
        }
        write!(out, "{edge}").map_err(|err| err.to_string())?;
    }
    writeln!(out).map_err(|err| err.to_string())
}

fn write_box_row<W: Write, I, S>(
    out: &mut W,
    cells: I,
    widths: &[usize],
    edge: char,
) -> Result<(), String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    write!(out, "{edge}").map_err(|err| err.to_string())?;
    for (index, cell) in cells.into_iter().enumerate() {
        let width = widths.get(index).copied().unwrap_or(0);
        write!(out, " {:<width$} {edge}", cell.as_ref(), width = width)
            .map_err(|err| err.to_string())?;
    }
    writeln!(out).map_err(|err| err.to_string())
}

fn write_markdown_row<W: Write, I, S>(out: &mut W, cells: I, widths: &[usize]) -> Result<(), String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    write!(out, "|").map_err(|err| err.to_string())?;
    for (index, cell) in cells.into_iter().enumerate() {
        let width = widths.get(index).copied().unwrap_or(0);
        write!(out, " {:<width$} |", cell.as_ref(), width = width)
            .map_err(|err| err.to_string())?;
    }
    writeln!(out).map_err(|err| err.to_string())
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
