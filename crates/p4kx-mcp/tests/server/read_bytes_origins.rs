// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{TestContainer, TestEntry};
use serde_json::{Value, json};

use crate::support::{Install, TestResult};

#[test]
fn an_encrypted_page_links_the_selected_bytes_without_marking_directory_values() -> TestResult {
    let path = "Data/fixture.bin";
    let install = Install::new(vec![
        TestEntry::deflated(path, b"0123456789".to_vec()).encrypted(),
    ])?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let reply = server.ok(
        "read_entry_bytes",
        &json!({"path": path, "offset": 2, "max_bytes": 3}),
    )?;
    assert_eq!(
        reply.get("data"),
        Some(&json!("MjM0")),
        "only the selected bytes"
    );
    for field in ["path", "size"] {
        assert_eq!(
            origin(&reply, field)?.get("containsEncryptedSource"),
            Some(&json!(false)),
            "{field} comes from the Archive directory"
        );
    }
    let data = origin(&reply, "data")?;
    assert_eq!(
        data.get("containsEncryptedSource"),
        Some(&json!(true)),
        "the page came from encrypted data"
    );
    let refs = data
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("page uses")?;
    let uses = reply
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("registered uses")?;
    assert!(
        uses.iter().any(|site| {
            site.get("id").is_some_and(|id| refs.contains(id))
                && site.pointer("/location/namespace") == Some(&json!("archiveEntryDecodedBytes"))
                && site.pointer("/location/indices") == Some(&json!(["2", "5"]))
        }),
        "the page addresses the exact original uncompressed range"
    );
    Ok(())
}

#[test]
fn an_empty_page_has_only_the_directory_bound_and_omits_the_next_offset_origin() -> TestResult {
    let path = "Data/fixture.bin";
    let install = Install::new(vec![TestEntry::stored(path, b"a".to_vec()).encrypted()])?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let reply = server.ok("read_entry_bytes", &json!({"path": path, "offset": 1}))?;
    assert_eq!(reply.get("data"), Some(&json!("")), "no bytes are returned");
    assert_eq!(
        origin(&reply, "data")?.get("containsEncryptedSource"),
        Some(&json!(false)),
        "the proven empty page contributes no encrypted payload"
    );
    assert!(
        origin(&reply, "next_offset").is_err(),
        "no absent next offset origin"
    );
    Ok(())
}

#[test]
fn an_encrypted_container_contributes_to_its_child_directory_values() -> TestResult {
    let container = TestContainer::new()
        .entry(TestEntry::stored("inner.bin", b"0123".to_vec()))
        .build();
    let install = Install::new(vec![
        TestEntry::zstd("Data/fixture.socpak", container).encrypted(),
    ])?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let reply = server.ok(
        "read_entry_bytes",
        &json!({"path": "data/fixture.socpak::INNER.bin"}),
    )?;
    assert_eq!(
        reply.get("path"),
        Some(&json!("Data/fixture.socpak::inner.bin")),
        "actual container and Entry spelling"
    );
    for field in ["path", "size", "data"] {
        assert_eq!(
            origin(&reply, field)?.get("containsEncryptedSource"),
            Some(&json!(true)),
            "{field} originates inside the encrypted container"
        );
    }
    let path_uses = origin(&reply, "path")?
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("path uses")?;
    assert_eq!(path_uses.len(), 2, "the path uses both directory names");
    Ok(())
}

#[test]
fn the_output_schema_declares_the_field_origins_and_their_exact_result_pointers() -> TestResult {
    let path = "fixture.bin";
    let install = Install::new(vec![TestEntry::stored(path, b"page".to_vec())])?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let tools = server.tools()?;
    let schema = &tools
        .iter()
        .find(|tool| tool["name"] == "read_entry_bytes")
        .ok_or("read bytes tool")?["outputSchema"];
    let fields = &schema["$defs"]["BytesFieldOrigins"];
    assert_eq!(fields["additionalProperties"], false);
    assert!(schema["properties"]["fieldOrigins"].is_object());
    assert!(schema["properties"]["provenance"].is_object());
    let required = fields["required"].as_array().ok_or("required origins")?;
    assert!(required.contains(&json!("/structuredContent/data")));
    assert!(!required.contains(&json!("/structuredContent/next_offset")));

    let response = server.call("read_entry_bytes", &json!({"path": path, "max_bytes": 2}))?;
    let origins = response["structuredContent"]["fieldOrigins"]
        .as_object()
        .ok_or("field origins")?;
    assert_eq!(origins.len(), 11);
    for pointer in origins.keys() {
        assert!(response.pointer(pointer).is_some(), "{pointer}");
    }
    Ok(())
}
fn origin<'reply>(reply: &'reply Value, field: &str) -> TestResult<&'reply Value> {
    reply
        .get("fieldOrigins")
        .and_then(|origins| origins.get(format!("/structuredContent/{field}")))
        .ok_or_else(|| format!("missing origin for {field}").into())
}
