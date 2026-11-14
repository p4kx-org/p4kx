// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::io::Write as _;
use std::ops::Range;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::line_sink::{Halt, LineSink};
use crate::{Excerpt, Needle};

type TestResult = Result<(), Box<dyn Error>>;

fn streamed(needle: &Needle, text: &str) -> Option<Range<u64>> {
    let mut scanner = needle.scanner();
    text.char_indices()
        .find_map(|(at, character)| scanner.push(character, at as u64))
}

#[test]
fn streamed_literals_preserve_original_character_start_and_expansion_end() -> TestResult {
    let dot = Needle::new("\u{307}")?;
    assert_eq!(
        dot.find("\u{130}"),
        None,
        "the existing contract cannot start inside lowercase expansion"
    );
    assert_eq!(
        streamed(&dot, "\u{130}"),
        None,
        "streamed matching must not invent that start"
    );
    for (pattern, text) in [
        ("i", "x\u{130}y"),
        ("\u{307}", "I\u{307}"),
        ("\u{307}i", "\u{130}i"),
        ("i\u{307}i", "I\u{130}i"),
        ("\u{3c3}\u{3c3}", "\u{3c2}\u{3a3}\u{3c3}"),
        (
            "\u{3b1}\u{3b2}\u{3b1}\u{3b2}",
            "\u{3b1}\u{3b2}\u{3b1}\u{3b2}\u{3b1}\u{3b2}",
        ),
        ("ababaca", "ababababaca"),
    ] {
        let needle = Needle::new(pattern)?;
        let expected = needle
            .find(text)
            .map(|range| range.start as u64..range.end as u64);
        assert_eq!(
            streamed(&needle, text),
            expected,
            "actual whole-text contract: {pattern:?} in {text:?}"
        );
    }
    Ok(())
}

fn split_matches(needle: &Needle, text: &str, split: usize) -> TestResult {
    let mut sink = LineSink::new(needle, "original.txt", 3, &|| false);
    sink.write_all(&text.as_bytes()[..split])?;
    sink.write_all(&text.as_bytes()[split..])?;
    let (hits, halt) = sink.finish();
    assert_eq!(
        halt, None,
        "the original UTF8 text is valid across split {split}"
    );
    match needle.find(text) {
        Some(range) => {
            assert_eq!(hits.len(), 1, "one original line at split {split}");
            let hit = hits.first().ok_or("actual matching source line")?;
            assert_eq!(
                (hit.line, hit.offset),
                (1, 0),
                "the same original line boundary"
            );
            assert_eq!(
                hit.excerpt,
                Excerpt::around(text, range),
                "exact original range/context at split {split}"
            );
        }
        None => assert!(
            hits.is_empty(),
            "no invented lowercase-expansion match at split {split}"
        ),
    }
    Ok(())
}

#[test]
fn every_byte_split_preserves_unicode_overlap_and_exact_excerpt() -> TestResult {
    let cases = [
        ("i", "left \u{130} right"),
        ("\u{307}", "left \u{130} right"),
        ("i\u{307}i", "left I\u{130}i right"),
        ("\u{3c3}\u{3c3}", "left \u{3c2}\u{3a3}\u{3c3} right"),
        (
            "\u{3b1}\u{3b2}\u{3b1}\u{3b2}",
            "left \u{3b1}\u{3b2}\u{3b1}\u{3b2}\u{3b1}\u{3b2} right",
        ),
    ];
    for (pattern, text) in cases {
        let needle = Needle::new(pattern)?;
        for split in 0..=text.len() {
            split_matches(&needle, text, split)?;
        }
    }
    let text = format!("{} needle {}", "\u{1f642}".repeat(40), "\u{3b1}".repeat(80));
    let needle = Needle::new("needle")?;
    for split in 0..=text.len() {
        split_matches(&needle, &text, split)?;
    }
    Ok(())
}

#[test]
fn a_literal_longer_than_the_excerpt_and_read_block_keeps_its_first_range() -> TestResult {
    let pattern = format!("{}\u{3b2}", "\u{3b1}".repeat(40_000));
    let text = format!("head {pattern} tail");
    let needle = Needle::new(&pattern)?;
    let range = needle
        .find(&text)
        .ok_or("actual complete original long literal")?;
    for block in [1, 2, 7, 4096, 65_536] {
        let mut sink = LineSink::new(&needle, "original.txt", 3, &|| false);
        for bytes in text.as_bytes().chunks(block) {
            sink.write_all(bytes)?;
        }
        let (hits, halt) = sink.finish();
        assert_eq!(
            halt, None,
            "the long original literal is valid at block {block}"
        );
        let hit = hits.first().ok_or("actual long original match")?;
        assert_eq!(hits.len(), 1, "one line, not one match per block");
        assert_eq!(
            hit.excerpt,
            Excerpt::around(&text, range.clone()),
            "old long-match ellipsis/context semantics"
        );
    }
    Ok(())
}

#[test]
fn a_single_large_write_is_cancellable_before_it_finishes() -> TestResult {
    let needle = Needle::new("needle")?;
    let polls = AtomicUsize::new(0);
    let stop = || polls.fetch_add(1, Ordering::Relaxed) == 1;
    let mut sink = LineSink::new(&needle, "original.txt", 3, &stop);
    assert!(
        sink.write_all(&vec![b'x'; 200_000]).is_err(),
        "actual second block observes cancellation"
    );
    let (hits, halt) = sink.finish();
    assert!(
        hits.is_empty(),
        "cancelled source work never commits a partial line"
    );
    assert_eq!(
        halt,
        Some(Halt::Cancelled),
        "stop is distinct from binary or an apparent no-match"
    );
    Ok(())
}

#[test]
fn converted_xml_keeps_internal_cr_and_omits_only_one_trailing_cr() -> TestResult {
    let needle = Needle::new("\r")?;
    for writes in [
        vec![b"tail\r\n".as_slice()],
        vec![b"tail\r".as_slice(), b"".as_slice(), b"\n".as_slice()],
    ] {
        let mut sink = LineSink::new(&needle, "original.xml", 3, &|| false).accepting_controls();
        for bytes in writes {
            sink.write_all(bytes)?;
        }
        let (hits, halt) = sink.finish();
        assert_eq!(halt, None, "converted XML retains the existing line policy");
        assert!(hits.is_empty(), "the one final CR is not a source match");
    }
    let mut sink = LineSink::new(&needle, "original.xml", 3, &|| false).accepting_controls();
    sink.write_all(b"tail\r")?;
    sink.write_all(b"\r\n")?;
    let (hits, halt) = sink.finish();
    assert_eq!(halt, None, "the original two-CR source remains valid");
    assert_eq!(
        hits.len(),
        1,
        "only the last CR is omitted; its predecessor remains inside the line"
    );
    Ok(())
}
