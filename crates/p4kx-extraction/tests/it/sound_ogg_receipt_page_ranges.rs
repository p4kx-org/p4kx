// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use serde_json::{Value, json};

use super::TestResult;

pub(super) fn assert_output_addresses(rows: &[Value], bytes: &[u8]) -> TestResult {
    let mut fields = rows.iter().filter_map(|row| row.get("address"));
    let mut offset = 0;
    for expected in ["identification", "commentsAndSetup", "audio"] {
        let span = fields.next().ok_or("one typed page-group origin")?;
        assert_eq!(span.get("type"), Some(&json!("oggBytes")), "Ogg address");
        assert_eq!(span.get("field"), Some(&json!(expected)), "addressed group");
        assert_eq!(
            span.get("start"),
            Some(&json!(offset.to_string())),
            "contiguous address"
        );
        let end: usize = span
            .get("endExclusive")
            .and_then(Value::as_str)
            .ok_or("group end")?
            .parse()?;
        let mut group = bytes.get(offset..end).ok_or("bytes in addressed group")?;
        assert!(
            !group.is_empty(),
            "each range has at least one complete page"
        );
        while !group.is_empty() {
            group = skip_page(group, expected)?;
        }
        offset = end;
    }
    assert!(fields.next().is_none(), "exactly three groups");
    assert_eq!(
        offset,
        bytes.len(),
        "groups cover complete output through EOS"
    );
    Ok(())
}

fn skip_page<'a>(bytes: &'a [u8], field: &str) -> Result<&'a [u8], Box<dyn Error>> {
    let header = bytes.get(..27).ok_or("Ogg page header")?;
    assert!(header.starts_with(b"OggS"), "address starts at a page");
    let count = usize::from(*header.last().ok_or("Ogg lacing count")?);
    let lacing = bytes.get(27..27 + count).ok_or("Ogg page lacing")?;
    let end = 27 + count + lacing.iter().map(|&size| usize::from(size)).sum::<usize>();
    let rest = bytes
        .get(end..)
        .ok_or("complete page within addressed group")?;
    if field == "audio" && rest.is_empty() {
        assert!(
            header.get(5).is_some_and(|flags| flags & 4 != 0),
            "audio ends with EOS"
        );
    }
    Ok(rest)
}
