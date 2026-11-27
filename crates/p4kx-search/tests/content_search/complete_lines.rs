// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_search::SearchOptions;

use crate::complete_lines_source::{self as source, CUT, Checked, READ_EDGE};

#[test]
fn complete_lines_find_a_match_after_one_mebibyte_without_a_final_break() -> Checked {
    let mut text = String::from("first\r\n");
    text.push_str(&"x".repeat(CUT + 67));
    text.push_str(" NeEdLe right");
    assert_eq!(
        text.find("NeEdLe"),
        Some(7 + CUT + 68),
        "the original literal occurs only after the former cut"
    );
    let fixture = source::fixture(text.as_bytes())?;
    let found = source::run(&fixture, "needle", &SearchOptions::default())?;
    source::complete(&found, 1);
    let hit = source::hit(&found, 0)?;
    assert_eq!(
        (hit.line, hit.offset),
        (2, 7),
        "the real line begins after the original CRLF"
    );
    assert_eq!(
        hit.excerpt.matched_text(),
        "NeEdLe",
        "case-insensitive matching keeps original text"
    );
    assert_eq!(
        hit.excerpt.before(),
        format!("\u{2026}{} ", "x".repeat(31)),
        "exact last32 original context characters"
    );
    assert_eq!(
        hit.excerpt.after(),
        " right",
        "real suffix is not silently cut"
    );
    Ok(())
}

#[test]
fn complete_lines_keep_utf8_split_at_a_read_boundary_and_the_former_cut() -> Checked {
    let mut text = "x".repeat(READ_EDGE - 1);
    text.push('\u{1f6f0}');
    text.push_str(&"x".repeat(CUT - text.len() - 1));
    text.push_str("\u{1f6f0}NeEdLe suffix");
    assert_eq!(
        text.get(READ_EDGE - 1..READ_EDGE + 3),
        Some("\u{1f6f0}"),
        "original four-byte scalar spans the64KiB read boundary"
    );
    assert_eq!(
        text.get(CUT - 1..CUT + 3),
        Some("\u{1f6f0}"),
        "another complete scalar spans the former cut"
    );
    let fixture = source::fixture(text.as_bytes())?;
    let found = source::run(&fixture, "\u{1f6f0}needle", &SearchOptions::default())?;
    source::complete(&found, 1);
    let hit = source::hit(&found, 0)?;
    assert_eq!(
        (hit.line, hit.offset),
        (1, 0),
        "the source still has one complete line"
    );
    assert_eq!(
        hit.excerpt.matched_text(),
        "\u{1f6f0}NeEdLe",
        "the match retains the original multibyte scalar"
    );
    assert_eq!(
        hit.excerpt.after(),
        " suffix",
        "UTF8 continuation bytes are not dropped"
    );
    Ok(())
}

#[test]
fn complete_lines_report_the_first_match_once_and_keep_later_line_offsets() -> Checked {
    let mut text = String::from("needle FIRST ");
    text.push_str(&"x".repeat(CUT + 19));
    text.push_str(" needle SECOND\r\n");
    let second = text.len() as u64;
    text.push_str("tail needle");
    assert_eq!(
        text.matches("needle").count(),
        3,
        "three original occurrences belong to two lines"
    );
    let fixture = source::fixture(text.as_bytes())?;
    let found = source::run(&fixture, "needle", &SearchOptions::default())?;
    source::complete(&found, 2);
    let first = source::hit(&found, 0)?;
    assert_eq!(
        (first.line, first.offset),
        (1, 0),
        "one hit for the entire first line"
    );
    assert_eq!(
        first.excerpt.before(),
        "",
        "the first original match starts the line"
    );
    assert_eq!(
        first.excerpt.after(),
        format!(" FIRST {}\u{2026}", "x".repeat(57)),
        "first-match context is exact and explicitly excerpted"
    );
    let last = source::hit(&found, 1)?;
    assert_eq!(
        (last.line, last.offset),
        (2, second),
        "the full first line contributes to the next offset"
    );
    assert_eq!(
        last.excerpt.before(),
        "tail ",
        "the second line retains its own original context"
    );
    Ok(())
}

#[test]
fn complete_lines_keep_crlf_across_read_boundaries_and_lone_cr_lines() -> Checked {
    let mut text = "x".repeat(READ_EDGE - 1);
    text.push_str("\r\n");
    let second = text.len() as u64;
    text.push_str(&"x".repeat(CUT + 5));
    text.push_str(" needle\r");
    let third = text.len() as u64;
    text.push_str("needle final\n");
    assert_eq!(
        text.as_bytes().get(READ_EDGE - 1..READ_EDGE + 1),
        Some(b"\r\n".as_slice()),
        "the original CRLF crosses the read boundary"
    );
    let fixture = source::fixture(text.as_bytes())?;
    let found = source::run(&fixture, "needle", &SearchOptions::default())?;
    source::complete(&found, 2);
    let positions: Vec<_> = found
        .hits
        .iter()
        .map(|hit| (hit.line, hit.offset))
        .collect();
    assert_eq!(
        positions,
        [(2, second), (3, third)],
        "CRLF is one break and loneCR is another"
    );
    Ok(())
}

#[test]
fn complete_lines_validate_invalid_utf8_after_an_early_match_and_the_former_cut() -> Checked {
    let mut bytes = b"needle ".to_vec();
    bytes.extend(std::iter::repeat_n(b'x', CUT + 31));
    let valid = source::fixture(&bytes)?;
    let complete = source::run(&valid, "needle", &SearchOptions::default())?;
    source::complete(&complete, 1);
    bytes.push(0xff);
    assert!(
        std::str::from_utf8(&bytes).is_err(),
        "the original tail really is invalid UTF8"
    );
    let invalid = source::fixture(&bytes)?;
    let found = source::run(&invalid, "needle", &SearchOptions::default())?;
    assert!(
        found.hits.is_empty(),
        "the full invalid line cannot produce an apparently valid match"
    );
    assert_eq!(
        found.stats.entries_binary, 1,
        "the actual unreadable UTF8 tail is counted as binary"
    );
    assert_eq!(
        found.stats.entries_scanned, 0,
        "the invalid Entry is not complete text"
    );
    assert!(
        !found.has_more,
        "binary classification is not result-limit truncation"
    );
    Ok(())
}

#[test]
fn complete_lines_search_the_complete_decoded_utf16_line() -> Checked {
    let mut text = "x".repeat(CUT + 11);
    text.push_str(" NeEdLe \u{1f6f0}");
    let mut bytes = vec![0xff, 0xfe];
    for unit in text.encode_utf16() {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    assert_eq!(
        String::from_utf16(&text.encode_utf16().collect::<Vec<_>>())?,
        text,
        "the independent original UTF16 text is valid"
    );
    let fixture = source::fixture(&bytes)?;
    let found = source::run(&fixture, "needle", &SearchOptions::default())?;
    source::complete(&found, 1);
    let hit = source::hit(&found, 0)?;
    assert_eq!(
        (hit.line, hit.offset),
        (1, 0),
        "decoded text uses its real first line without the BOM"
    );
    assert_eq!(
        hit.excerpt.matched_text(),
        "NeEdLe",
        "the complete decoded suffix is searched"
    );
    assert_eq!(
        hit.excerpt.after(),
        " \u{1f6f0}",
        "the surrogate pair remains intact"
    );
    Ok(())
}
