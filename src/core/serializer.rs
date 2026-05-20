use super::model::Entry;

pub fn serialize(entries: &[Entry]) -> String {
    let mut output = String::new();
    for entry in entries {
        if entry.disabled {
            output.push_str("# ");
        }
        output.push_str(&entry.ip);
        output.push(' ');
        output.push_str(&entry.hostnames.join(" "));
        if let Some(ref c) = entry.comment {
            if !c.is_empty() {
                output.push_str(" # ");
                output.push_str(c);
            }
        }
        output.push('\n');
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::parser::parse;

    #[test]
    fn test_round_trip() {
        let input = "127.0.0.1 localhost\n::1 localhost\n10.0.0.1 app.local api.local # dev\n";
        let entries = parse(input);
        let output = serialize(&entries);
        let reparsed = parse(&output);
        assert_eq!(reparsed.len(), 3);
    }
}
