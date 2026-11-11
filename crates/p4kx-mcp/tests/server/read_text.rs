// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{CryXmlByteOrder, CryXmlSignature, TestContainer, TestElement, TestEntry};
use serde_json::{Value, json};

use crate::support::{Install, Server, TestResult};

fn install() -> TestResult<Install> {
    let ship = TestElement::new("Ship")
        .attribute("name", "Gladius")
        .child(TestElement::new("Part").attribute("class", "wing"))
        .child(TestElement::new("Seat").content("pilot"))
        .to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Big);
    let mut huge = b"CryXmlB\0".to_vec();
    huge.resize((32 << 20) + 1, 0);
    let mut long = "ä".repeat(5000).into_bytes();
    long.extend_from_slice(b"end");
    Install::new(vec![
        TestEntry::zstd("Data/ship.xml", ship),
        TestEntry::zstd("Data/huge.xml", huge),
        TestEntry::stored("Data/be.txt", b"\xFE\xFF\x00h\x00\xE4\x00x".to_vec()),
        TestEntry::stored("Data/le.txt", b"\xFF\xFEh\x00\xE4\x00x\x00".to_vec()),
        TestEntry::stored("Data/odd.txt", b"\xFF\xFEh\x00\xE4".to_vec()),
        TestEntry::stored("Data/surrogate.txt", b"\xFF\xFE\x00\xD8h\x00".to_vec()),
        TestEntry::stored("Data/cut.txt", b"abc\xC3".to_vec()),
        TestEntry::stored("Data/control.txt", b"a\x01b".to_vec()),
        TestEntry::stored("Data/delete.txt", b"a\x7Fb".to_vec()),
        TestEntry::stored("Data/latin1.txt", b"gr\xFC\xDFe".to_vec()),
        TestEntry::stored("Data/feed.txt", b"a\x0Cb\tc\r\n".to_vec()),
        TestEntry::deflated("Data/long.txt", long),
    ])
}

fn whole_text(server: &mut Server, path: &str, max_bytes: u32) -> TestResult<String> {
    let mut text = String::new();
    let mut offset = Value::from(0);
    loop {
        let page = server.ok(
            "read_entry_text",
            &json!({ "path": path, "offset": offset, "max_bytes": max_bytes }),
        )?;
        assert_eq!(
            page.get("offset"),
            Some(&offset),
            "each page starts where the last ended"
        );
        text.push_str(page.get("text").and_then(Value::as_str).unwrap_or_default());
        match page.get("next_offset") {
            Some(next) => offset = next.clone(),
            None => return Ok(text),
        }
    }
}

#[test]
fn paging_a_cryxml_to_its_end_joins_back_into_the_whole_xml() -> TestResult {
    let install = install()?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let whole = server.ok("read_entry_text", &json!({ "path": "Data/ship.xml" }))?;
    let xml = whole["text"].as_str().unwrap_or_default();
    assert_eq!(
        xml,
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Ship name=\"Gladius\">\n  \
         <Part class=\"wing\"/>\n  <Seat>pilot</Seat>\n</Ship>\n",
        "the converted XML"
    );
    assert_eq!(whole["size"], xml.len(), "the size of the whole XML");
    assert_eq!(
        whole_text(&mut server, "Data/ship.xml", 7)?,
        xml,
        "joined pages"
    );
    let long = whole_text(&mut server, "Data/long.txt", 4097)?;
    assert_eq!(
        long,
        format!("{}end", "ä".repeat(5000)),
        "UTF-8 pages beyond the first 8 KiB join back"
    );
    Ok(())
}

#[test]
fn utf16_of_both_byte_orders_reads_from_any_offset() -> TestResult {
    let install = install()?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    for (path, encoding) in [("Data/be.txt", "utf-16be"), ("Data/le.txt", "utf-16le")] {
        let all = server.ok("read_entry_text", &json!({ "path": path }))?;
        assert_eq!(
            (&all["encoding"], &all["text"], &all["size"]),
            (&json!(encoding), &json!("häx"), &json!(4)),
            "{path}"
        );
        let inside = server.ok("read_entry_text", &json!({ "path": path, "offset": 2 }))?;
        assert_eq!(
            (&inside["offset"], &inside["text"]),
            (&json!(3), &json!("x")),
            "{path}: an offset inside ä moves behind it"
        );
    }
    Ok(())
}

#[test]
fn malformed_and_control_character_text_is_binary() -> TestResult {
    let install = install()?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    for path in [
        "Data/odd.txt",
        "Data/surrogate.txt",
        "Data/cut.txt",
        "Data/control.txt",
        "Data/delete.txt",
        "Data/latin1.txt",
    ] {
        let message = server.error("read_entry_text", &json!({ "path": path }))?;
        assert!(message.contains("binary data"), "{path}: {message}");
    }
    let feed = server.ok("read_entry_text", &json!({ "path": "Data/feed.txt" }))?;
    assert_eq!(
        feed["text"], "a\u{c}b\tc\r\n",
        "tab, line breaks and form feed are text"
    );
    Ok(())
}

#[test]
fn a_cryxml_too_large_to_decode_points_to_the_bytes_tool() -> TestResult {
    let install = install()?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let message = server.error("read_entry_text", &json!({ "path": "Data/huge.xml" }))?;
    assert!(
        message.contains("too large") && message.contains("read_entry_bytes"),
        "{message}"
    );
    let bytes = server.ok(
        "read_entry_bytes",
        &json!({ "path": "Data/huge.xml", "offset": 32 << 20, "max_bytes": 8 }),
    )?;
    assert_eq!(
        bytes["data"], "AA==",
        "the last byte through the bytes tool"
    );
    Ok(())
}

#[test]
fn an_encrypted_utf8_page_identifies_the_selected_original_bytes_after_bom_and_alignment()
-> TestResult {
    let path = "page.txt";
    let entry = TestEntry::deflated(path, "\u{feff}AäBCDEF".as_bytes().to_vec()).encrypted();
    let install = Install::new(vec![entry])?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let result = server.ok(
        "read_entry_text",
        &json!({"path": path, "offset": 2, "max_bytes": 4}),
    )?;
    assert_eq!(result["text"], "BCDE");
    assert_eq!(result["offset"], 3);
    assert_eq!(result["next_offset"], 7);
    let fields = &result["fieldOrigins"];
    assert_eq!(
        fields["/structuredContent/text"]["containsEncryptedSource"],
        true
    );
    assert_eq!(
        fields["/structuredContent/path"]["containsEncryptedSource"],
        false
    );
    assert_eq!(
        fields["/structuredContent/max_bytes"]["containsEncryptedSource"],
        false
    );
    assert!(has_text_range(&result, "utf8Text", "6", "10"));
    assert!(has_text_range(&result, "characterBoundaries", "5", "10"));
    Ok(())
}

#[test]
fn transcoded_utf16_addresses_the_original_units_even_when_the_utf8_window_is_shorter() -> TestResult
{
    let path = "utf16.txt";
    let mut bytes = vec![0xff, 0xfe];
    for unit in "A😀Z".encode_utf16() {
        bytes.extend(unit.to_le_bytes());
    }
    let install = Install::new(vec![TestEntry::deflated(path, bytes).encrypted()])?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let result = server.ok(
        "read_entry_text",
        &json!({"path": path, "offset": 2, "max_bytes": 4}),
    )?;
    assert_eq!(result["text"], "Z");
    assert_eq!(result["offset"], 5);
    assert_eq!(result["size"], 6);
    let fields = &result["fieldOrigins"];
    assert_eq!(
        fields["/structuredContent/text"]["containsEncryptedSource"],
        true
    );
    assert!(fields.get("/structuredContent/next_offset").is_none());
    assert!(has_text_range(&result, "transcodeInput", "0", "10"));
    Ok(())
}

fn has_text_range(result: &Value, field: &str, start: &str, end: &str) -> bool {
    let Some(refs) = result
        .pointer("/fieldOrigins/~1structuredContent~1text/sourceUsesRefs")
        .and_then(Value::as_array)
    else {
        return false;
    };
    result
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .is_some_and(|uses| {
            uses.iter().any(|usage| {
                usage.get("id").is_some_and(|id| refs.contains(id))
                    && usage.pointer("/location/namespace")
                        == Some(&json!("archiveEntryDecodedBytes"))
                    && usage.pointer("/location/field") == Some(&json!(field))
                    && usage.pointer("/location/indices") == Some(&json!([start, end]))
            })
        })
}

#[test]
fn cryxml_text_retains_the_original_attribute_from_the_page_and_its_encrypted_container()
-> TestResult {
    let original = TestElement::new("Node")
        .attribute("first", "match")
        .attribute("second", "extra")
        .to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little);
    let container = TestContainer::new()
        .entry(TestEntry::stored("node.xml", original))
        .build();
    let install = Install::new(vec![TestEntry::zstd("outer.socpak", container).encrypted()])?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let path = "outer.socpak::node.xml";
    let prefix = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Node first=\"";
    let result = server.ok(
        "read_entry_text",
        &json!({"path": path, "offset": prefix.len(), "max_bytes": 5}),
    )?;
    assert_eq!(result["text"], "match");
    let fields = &result["fieldOrigins"];
    for field in ["path", "text", "size"] {
        let origin = &fields[format!("/structuredContent/{field}")];
        assert_eq!(origin["containsEncryptedSource"], true, "{field}");
        assert_eq!(origin["sourceSetComplete"], true, "{field}");
    }
    let refs = fields["/structuredContent/text"]["sourceUsesRefs"]
        .as_array()
        .ok_or("retained source uses")?;
    let attrs: Vec<_> = result["provenance"]["uses"]
        .as_array()
        .ok_or("source inventory")?
        .iter()
        .filter(|entry| {
            refs.contains(&entry["id"]) && entry["location"]["namespace"] == "cryxmlAttribute"
        })
        .collect();
    assert_eq!(attrs.len(), 1, "the other attribute is outside the page");
    assert_eq!(attrs[0]["location"]["field"], "value");
    assert_eq!(attrs[0]["location"]["indices"], json!(["0", "0"]));
    Ok(())
}

#[test]
fn the_text_origin_schema_has_exact_pointers_and_preserves_unknown_payload_origin() -> TestResult {
    let path = "unknown.txt";
    let entry = TestEntry::deflated(path, b"page".to_vec())
        .encrypted()
        .with_encryption_field(7);
    let install = Install::new(vec![entry])?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let tools = server.tools()?;
    let tool = tools
        .iter()
        .find(|tool| tool["name"] == "read_entry_text")
        .ok_or("text tool")?;
    let schema = &tool["outputSchema"];
    let fields = &schema["$defs"]["TextFieldOrigins"];
    assert_eq!(fields["additionalProperties"], false);
    assert!(schema["properties"]["provenance"].is_object());
    let required = fields["required"]
        .as_array()
        .ok_or("required field origins")?;
    assert!(required.contains(&json!("/structuredContent/text")));
    assert!(!required.contains(&json!("/structuredContent/next_offset")));

    let response = server.call("read_entry_text", &json!({"path": path}))?;
    let result = &response["structuredContent"];
    assert_eq!(result["text"], "page");
    let origins = result["fieldOrigins"].as_object().ok_or("field origins")?;
    for pointer in origins.keys() {
        assert!(response.pointer(pointer).is_some(), "{pointer}");
    }
    let text = &origins["/structuredContent/text"];
    assert_eq!(text.get("containsEncryptedSource"), Some(&Value::Null));
    assert_eq!(text["sourceSetComplete"], true);
    Ok(())
}
