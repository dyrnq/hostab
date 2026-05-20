use crate::core::model::Row;
use tabled::{
    settings::{object::Rows, Modify, Style, Width},
    Table, Tabled,
};

#[derive(Tabled)]
struct RowDisplay {
    ip: String,
    host: String,
    comment: String,
}

pub fn format_table(rows: &[Row]) -> String {
    let display_rows: Vec<RowDisplay> = rows
        .iter()
        .map(|r| RowDisplay {
            ip: r.ip.clone(),
            host: r.host.clone(),
            comment: r.comment.clone().unwrap_or_default(),
        })
        .collect();
    let mut table = Table::new(&display_rows);
    table
        .with(Style::rounded())
        .with(Modify::new(Rows::new(..)).with(Width::wrap(40)));
    table.to_string()
}

pub fn format_raw(rows: &[Row]) -> String {
    let mut output = String::from("IP\tHOST\tCOMMENT\n");
    for row in rows {
        output.push_str(&format!(
            "{}\t{}\t{}\n",
            row.ip,
            row.host,
            row.comment.as_deref().unwrap_or("")
        ));
    }
    output
}

pub fn format_markdown(rows: &[Row]) -> String {
    let mut output = String::from("| IP | Host | Comment |\n");
    output.push_str("|----|------|--------|\n");
    for row in rows {
        output.push_str(&format!(
            "| {} | {} | {} |\n",
            row.ip,
            row.host,
            row.comment.as_deref().unwrap_or("")
        ));
    }
    output
}

pub fn format_status(_rows: &[Row]) -> String {
    String::new()
}
