use crate::core::model::Row;
use tabled::{settings::Style, Table, Tabled};

#[derive(Tabled)]
struct RowDisplay {
    ip: String,
    host: String,
    comment: String,
}

#[derive(Tabled)]
struct RowDisplayWithCanonical {
    ip: String,
    host: String,
    comment: String,
    #[tabled(rename = "canonical")]
    canonical_col: String,
}

pub fn format_table(rows: &[Row]) -> String {
    let has_canonical = rows.iter().any(|r| r.canonical.is_some());
    if has_canonical {
        let display_rows: Vec<RowDisplayWithCanonical> = rows
            .iter()
            .map(|r| RowDisplayWithCanonical {
                ip: r.ip.clone(),
                host: r.host.clone(),
                comment: r.comment.clone().unwrap_or_default(),
                canonical_col: r.canonical.clone().unwrap_or_default(),
            })
            .collect();
        let mut table = Table::new(&display_rows);
        table.with(Style::rounded());
        table.to_string()
    } else {
        let display_rows: Vec<RowDisplay> = rows
            .iter()
            .map(|r| RowDisplay {
                ip: r.ip.clone(),
                host: r.host.clone(),
                comment: r.comment.clone().unwrap_or_default(),
            })
            .collect();
        let mut table = Table::new(&display_rows);
        table.with(Style::rounded());
        table.to_string()
    }
}

pub fn format_raw(rows: &[Row]) -> String {
    let mut output = String::from("IP\tHOST\tCOMMENT\tCANONICAL\n");
    for row in rows {
        output.push_str(&format!(
            "{}\t{}\t{}\t{}\n",
            row.ip,
            row.host,
            row.comment.as_deref().unwrap_or(""),
            row.canonical.as_deref().unwrap_or("")
        ));
    }
    output
}

pub fn format_markdown(rows: &[Row]) -> String {
    let mut output = String::from("| IP | Host | Comment | Canonical |\n");
    output.push_str("|----|------|---------|-----------|\n");
    for row in rows {
        output.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            row.ip,
            row.host,
            row.comment.as_deref().unwrap_or(""),
            row.canonical.as_deref().unwrap_or("")
        ));
    }
    output
}

pub fn format_status(_rows: &[Row]) -> String {
    String::new()
}
