// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{CryXmlByteOrder, CryXmlSignature, TestElement, TestEntry};
use serde_json::{Value, json};

use crate::support::{Install, TestResult};

#[test]
fn bytes_come_as_base64_from_an_offset_with_a_maximum_length() -> TestResult {
    let install = Install::new(vec![TestEntry::deflated(
        "Data/Objects/ship.cgf",
        b"0123456789".to_vec(),
    )])?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let mut first = server.ok(
        "read_entry_bytes",
        &json!({ "path": r"data\OBJECTS\ship.cgf", "offset": 2, "max_bytes": 5 }),
    )?;
    let payload = first.as_object_mut().ok_or("structured byte result")?;
    for member in ["fieldOrigins", "provenance"] {
        assert!(
            payload.remove(member).is_some(),
            "the result contains {member}"
        );
    }
    assert_eq!(
        first,
        json!({
            "archive": install.archive().display().to_string(),
            "path": "Data/Objects/ship.cgf",
            "size": 10,
            "offset": 2,
            "length": 5,
            "encoding": "base64",
            "data": "MjM0NTY=",
            "more": true,
            "next_offset": 7,
            "clamped": false,
            "max_bytes": 5
        }),
        "bytes 2 to 6"
    );
    let rest = server.ok(
        "read_entry_bytes",
        &json!({ "path": "Data/Objects/ship.cgf", "offset": 7 }),
    )?;
    assert_eq!(rest["data"], "Nzg5", "the last three bytes");
    assert!(rest.get("next_offset").is_none(), "nothing follows: {rest}");
    let end = server.ok(
        "read_entry_bytes",
        &json!({ "path": "Data/Objects/ship.cgf", "offset": 10 }),
    )?;
    assert_eq!(end["length"], 0, "the end is empty");
    let message = server.error(
        "read_entry_bytes",
        &json!({ "path": "Data/Objects/ship.cgf", "offset": 11 }),
    )?;
    assert!(message.contains("beyond the end"), "{message}");
    Ok(())
}

const SHIP_XML: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<Ship name=\"Gladius\">\n  <Part class=\"wing\"/>\n</Ship>\n";

fn texts() -> TestResult<Install> {
    let ship = TestElement::new("Ship")
        .attribute("name", "Gladius")
        .child(TestElement::new("Part").attribute("class", "wing"))
        .to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little);
    let mut binary_later = b"x".repeat(9000);
    binary_later.extend_from_slice(b"\0\x01\x02");
    Install::new(vec![
        TestEntry::zstd("Data/Libs/notes.txt", "grüße\r\nzeile".as_bytes().to_vec()).encrypted(),
        TestEntry::deflated("Data/Libs/ship.xml", ship),
        TestEntry::stored("Data/Libs/wide.txt", b"\xFF\xFEh\x00\xE4\x00".to_vec()),
        TestEntry::stored("Data/Libs/bom.txt", b"\xEF\xBB\xBFabc".to_vec()),
        TestEntry::stored("Data/Objects/ship.cgf", b"CrCh\0\0\x01\x02".to_vec()),
        TestEntry::stored("Data/Objects/late.bin", binary_later),
        TestEntry::stored("Data/Objects/big.bin", vec![7; 300_000]),
        TestEntry::deflated("Data/Libs/big.txt", b"a".repeat(300_000)),
    ])
}

#[test]
fn text_comes_from_an_offset_with_a_maximum_length_and_says_whether_more_follows() -> TestResult {
    let install = texts()?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let first = server.ok(
        "read_entry_text",
        &json!({ "path": "data/libs/NOTES.txt", "max_bytes": 5 }),
    )?;
    assert_eq!(
        first,
        json!({
            "archive": install.archive().display().to_string(),
            "path": "Data/Libs/notes.txt",
            "encoding": "utf-8",
            "size": 14,
            "offset": 0,
            "length": 4,
            "text": "grü",
            "more": true,
            "next_offset": 4,
            "clamped": false,
            "max_bytes": 5
        }),
        "the range ends before a split character"
    );
    let rest = server.ok(
        "read_entry_text",
        &json!({ "path": "Data/Libs/notes.txt", "offset": 4 }),
    )?;
    assert_eq!(rest["text"], "ße\r\nzeile", "the rest");
    assert_eq!(rest["more"], false, "nothing follows");
    assert!(rest.get("next_offset").is_none(), "no next offset: {rest}");
    let inside = server.ok(
        "read_entry_text",
        &json!({ "path": "Data/Libs/notes.txt", "offset": 3, "max_bytes": 4 }),
    )?;
    assert_eq!(
        (&inside["offset"], &inside["text"]),
        (&json!(4), &json!("ße\r")),
        "an offset inside a character moves to the next one"
    );
    let bom = server.ok("read_entry_text", &json!({ "path": "Data/Libs/bom.txt" }))?;
    assert_eq!(
        (&bom["text"], &bom["size"]),
        (&json!("abc"), &json!(3)),
        "the byte order mark is dropped"
    );
    Ok(())
}

#[test]
fn cryxml_is_converted_and_utf16_is_decoded() -> TestResult {
    let install = texts()?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let ship = server.ok("read_entry_text", &json!({ "path": "Data/Libs/ship.xml" }))?;
    assert_eq!(ship["encoding"], "cryxml", "converted");
    assert_eq!(ship["text"], SHIP_XML, "the XML");
    let part = server.ok(
        "read_entry_text",
        &json!({ "path": "Data/Libs/ship.xml", "offset": 61, "max_bytes": 22 }),
    )?;
    assert_eq!(
        part["text"], "  <Part class=\"wing\"/>",
        "a range of the XML"
    );
    let wide = server.ok("read_entry_text", &json!({ "path": "Data/Libs/wide.txt" }))?;
    assert_eq!(
        (&wide["encoding"], &wide["text"], &wide["size"]),
        (&json!("utf-16le"), &json!("hä"), &json!(3)),
        "UTF-16 decoded, measured in UTF-8"
    );
    Ok(())
}

#[test]
fn requests_above_the_size_limit_are_clamped_and_say_so() -> TestResult {
    let install = texts()?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let bytes = server.ok(
        "read_entry_bytes",
        &json!({ "path": "Data/Objects/big.bin", "max_bytes": 1_000_000 }),
    )?;
    assert_eq!(
        (&bytes["clamped"], &bytes["max_bytes"], &bytes["length"]),
        (&json!(true), &json!(196_608), &json!(196_608)),
        "bytes clamped to 192 KiB"
    );
    assert_eq!(bytes["next_offset"], 196_608, "the rest follows");
    let text = server.ok(
        "read_entry_text",
        &json!({ "path": "Data/Libs/big.txt", "max_bytes": u32::MAX }),
    )?;
    assert_eq!(
        (&text["clamped"], &text["max_bytes"], &text["length"]),
        (&json!(true), &json!(262_144), &json!(262_144)),
        "text clamped to 256 KiB"
    );
    assert_eq!(text["more"], true, "the rest follows");
    let small = server.ok(
        "read_entry_text",
        &json!({ "path": "Data/Libs/notes.txt", "max_bytes": 1 }),
    )?;
    assert_eq!(
        (&small["clamped"], &small["max_bytes"], &small["text"]),
        (&json!(true), &json!(4), &json!("grü")),
        "raised to one whole character"
    );
    let default = server.ok(
        "read_entry_bytes",
        &json!({ "path": "Data/Objects/big.bin" }),
    )?;
    assert_eq!(
        (&default["clamped"], &default["length"]),
        (&json!(false), &json!(16_384)),
        "16 KiB by default"
    );
    Ok(())
}

#[test]
fn unknown_entries_and_binary_content_are_tool_errors() -> TestResult {
    let install = texts()?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    for tool in ["read_entry_text", "read_entry_bytes"] {
        let message = server.error(tool, &json!({ "path": "Data/Missing.xml" }))?;
        assert!(
            message.contains("no Entry `Data/Missing.xml`"),
            "{tool}: {message}"
        );
        let message = server.error(tool, &json!({ "path": "x".repeat(2000) }))?;
        assert!(message.contains("longer than"), "{tool}: {message}");
    }
    let message = server.error(
        "read_entry_text",
        &json!({ "path": "Data/Objects/ship.cgf" }),
    )?;
    assert!(
        message.contains("binary data") && message.contains("read_entry_bytes"),
        "{message}"
    );
    let message = server.error(
        "read_entry_text",
        &json!({ "path": "Data/Objects/late.bin", "offset": 8990 }),
    )?;
    assert!(
        message.contains("binary data"),
        "binary after the start: {message}"
    );
    let message = server.error(
        "read_entry_text",
        &json!({ "path": "Data/Libs/notes.txt", "offset": 15 }),
    )?;
    assert!(message.contains("beyond the end"), "{message}");
    let response = server.request(
        "tools/call",
        &json!({ "name": "read_entry_bytes", "arguments": { "path": "readme.txt", "length": 4 } }),
    )?;
    let rejected = response.get("error").is_some()
        || response.pointer("/result/isError") == Some(&Value::Bool(true));
    assert!(rejected, "an unknown argument fails: {response}");
    Ok(())
}
