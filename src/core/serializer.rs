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

    #[test]
    fn test_serialize_empty() {
        assert_eq!(serialize(&[]), "");
    }

    #[test]
    fn test_serialize_disabled() {
        let entries = vec![Entry {
            id: 1,
            ip: "10.0.0.1".into(),
            hostnames: vec!["blocked.local".into()],
            comment: None,
            disabled: true,
            raw: None,
        }];
        assert_eq!(serialize(&entries), "# 10.0.0.1 blocked.local\n");
    }

    #[test]
    fn test_serialize_with_comment() {
        let entries = vec![Entry {
            id: 1,
            ip: "10.0.0.1".into(),
            hostnames: vec!["app.local".into()],
            comment: Some("dev server".into()),
            disabled: false,
            raw: None,
        }];
        assert_eq!(serialize(&entries), "10.0.0.1 app.local # dev server\n");
    }

    #[test]
    fn test_serialize_empty_comment() {
        let entries = vec![Entry {
            id: 1,
            ip: "10.0.0.1".into(),
            hostnames: vec!["app.local".into()],
            comment: Some("".into()),
            disabled: false,
            raw: None,
        }];
        assert_eq!(serialize(&entries), "10.0.0.1 app.local\n");
    }

    #[test]
    fn test_serialize_mixed_enabled_disabled() {
        let entries = vec![
            Entry {
                id: 1,
                ip: "10.0.0.1".into(),
                hostnames: vec!["enabled.local".into()],
                comment: None,
                disabled: false,
                raw: None,
            },
            Entry {
                id: 2,
                ip: "10.0.0.2".into(),
                hostnames: vec!["disabled.local".into()],
                comment: None,
                disabled: true,
                raw: None,
            },
        ];
        assert_eq!(
            serialize(&entries),
            "10.0.0.1 enabled.local\n# 10.0.0.2 disabled.local\n"
        );
    }

    #[test]
    fn test_round_trip_disabled() {
        let input = "127.0.0.1 localhost\n# 10.0.0.1 blocked.local\n";
        let entries = parse(input);
        assert_eq!(entries.len(), 2);
        let output = serialize(&entries);
        let reparsed = parse(&output);
        assert_eq!(reparsed.len(), 2);
        assert!(!reparsed[0].disabled);
        assert!(reparsed[1].disabled);
    }

    #[test]
    fn test_round_trip_disabled_with_comment() {
        let input = "127.0.0.1 localhost\n# 10.0.0.1 blocked.local # reason\n";
        let entries = parse(input);
        assert_eq!(entries.len(), 2);
        let output = serialize(&entries);
        let reparsed = parse(&output);
        assert_eq!(reparsed.len(), 2);
        assert!(reparsed[1].disabled);
        assert_eq!(reparsed[1].comment, Some("reason".to_string()));
    }
}
