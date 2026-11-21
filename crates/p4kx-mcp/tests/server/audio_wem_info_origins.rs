// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{TestBank, TestCodec, TestEntry, TestWem};
use serde_json::{Value, json};

use crate::audio::serve;
use crate::audio_origin_assertions::{assert_marker, origin, physical_source, source_path, uses};
use crate::support::{Install, Server, TestResult};

const TOOL: &str = "get_wem_info";
const BAD_LOOSE: &str = "Data/Sounds/wwise/8100.wem";
const CLEAR_BANK: &str = "Data/Sounds/wwise/Ready.bnk";
const PREFERRED_LOOSE: &str = "Data/Sounds/wwise/8101.wem";
const ENCRYPTED_BANK: &str = "Data/Sounds/wwise/Protected.bnk";
const RATE: &str = "/structuredContent/wem/sample_rate";

fn pcm(sample_rate: u32) -> Vec<u8> {
    TestWem::pcm(TestCodec::Pcm, 1, sample_rate, &[0, 1, 2, 3]).bytes()
}

#[test]
fn wem_info_places_encrypted_failed_attempts_outside_clear_decoded_fields() -> TestResult {
    let bank = TestBank::new(1)
        .wem(7, pcm(22050))
        .wem(8100, pcm(16000))
        .bytes();
    let install = Install::new(vec![
        TestEntry::deflated(BAD_LOOSE, b"RIFF\x04\x00\x00\x00WAVE").encrypted(),
        TestEntry::stored(CLEAR_BANK, bank),
    ])?;
    let mut server = serve(&install)?;
    let first = server.ok(TOOL, &json!({"wem_id": 8100, "limit": 1}))?;
    assert_eq!(
        first.get("locations_total"),
        Some(&json!(2)),
        "the malformed loose WEM is still listed"
    );
    assert_location(&first, "/locations/0/path", BAD_LOOSE);
    assert_marker(&first, "/structuredContent/locations/0/path", false)?;
    assert_described(&first, CLEAR_BANK, 16000, false)?;
    assert_embedded_rate(&first)?;
    assert_schema(&mut server, &first)?;

    let cursor = first
        .get("next_cursor")
        .and_then(Value::as_str)
        .ok_or("next WEM location page")?;
    origin(&first, "/structuredContent/next_cursor")?;
    let second = server.ok(TOOL, &json!({"wem_id": 8100, "limit": 1, "cursor": cursor}))?;
    assert_location(&second, "/locations/0/path", CLEAR_BANK);
    assert_described(&second, CLEAR_BANK, 16000, false)?;
    assert!(
        second.get("next_cursor").is_none(),
        "both locations were delivered"
    );
    assert_receipt(&first)?;
    assert_receipt(&second)
}

#[test]
fn wem_info_keeps_preferred_loose_metadata_independent_of_an_encrypted_alternative() -> TestResult {
    let bank = TestBank::new(2)
        .wem(8101, pcm(16000))
        .wem(8102, pcm(22050))
        .bytes();
    let install = Install::new(vec![
        TestEntry::stored(PREFERRED_LOOSE, pcm(8000)),
        TestEntry::deflated(ENCRYPTED_BANK, bank).encrypted(),
    ])?;
    let mut server = serve(&install)?;
    let preferred = server.ok(TOOL, &json!({"wem_id": 8101}))?;
    assert_location(&preferred, "/locations/0/path", PREFERRED_LOOSE);
    assert_location(&preferred, "/locations/1/path", ENCRYPTED_BANK);
    assert_marker(&preferred, "/structuredContent/locations/1/path", false)?;
    assert_didx_offset(
        &preferred,
        "/structuredContent/locations/1/offset",
        ["64", "68"],
    )?;
    assert_described(&preferred, PREFERRED_LOOSE, 8000, false)?;
    assert_receipt(&preferred)?;

    let encrypted = server.ok(TOOL, &json!({"wem_id": 8102}))?;
    assert_described(&encrypted, ENCRYPTED_BANK, 22050, true)?;
    assert_didx_offset(
        &encrypted,
        "/structuredContent/described/offset",
        ["76", "80"],
    )?;
    assert_marker(&encrypted, "/structuredContent/described/len", true)?;
    assert_embedded_rate(&encrypted)?;
    assert_receipt(&encrypted)
}

fn assert_location(reply: &Value, field: &str, path: &str) {
    assert_eq!(
        reply.pointer(field).and_then(Value::as_str),
        Some(path),
        "physical location at {field}"
    );
}

fn assert_described(reply: &Value, path: &str, rate: u64, encrypted: bool) -> TestResult {
    assert_location(reply, "/described/path", path);
    assert_eq!(
        reply.pointer("/wem/sample_rate").and_then(Value::as_u64),
        Some(rate),
        "sample rate belongs to the chosen WEM"
    );
    assert_marker(reply, "/structuredContent/described/path", false)?;
    for field in ["/structuredContent/wem/codec", RATE] {
        assert_marker(reply, field, encrypted)?;
        let contributions = uses(reply, field)?;
        assert!(!contributions.is_empty(), "{field} has an observed source");
        for site in contributions {
            assert_eq!(
                source_path(reply, site)?,
                path,
                "{field} uses its chosen WEM"
            );
        }
    }
    Ok(())
}

fn assert_didx_offset(reply: &Value, field: &str, bytes: [&str; 2]) -> TestResult {
    assert_marker(reply, field, true)?;
    let site = uses(reply, field)?
        .into_iter()
        .find(|site| {
            site.pointer("/location/namespace").and_then(Value::as_str) == Some("bnkDidx")
                && site.pointer("/location/field").and_then(Value::as_str) == Some("dataOffset")
        })
        .ok_or("original DIDX offset word")?;
    assert_eq!(
        source_path(reply, site)?,
        ENCRYPTED_BANK,
        "{field} uses the physical bank"
    );
    assert_eq!(
        site.pointer("/location/indices"),
        Some(&json!(bytes)),
        "{field} identifies its own DIDX word"
    );
    Ok(())
}

fn assert_embedded_rate(reply: &Value) -> TestResult {
    let site = uses(reply, RATE)?
        .into_iter()
        .find(|site| {
            site.pointer("/location/namespace").and_then(Value::as_str) == Some("bnkWem.wemChunk")
                && site.pointer("/location/field").and_then(Value::as_str) == Some("sampleRate")
        })
        .ok_or("original embedded sample rate word")?;
    let positions = site
        .pointer("/location/indices")
        .and_then(Value::as_array)
        .ok_or("original source positions")?;
    let [start, end, format_offset, ..] = positions.as_slice() else {
        return Err("missing original bank and WEM positions".into());
    };
    assert_eq!(
        (start.as_str(), end.as_str(), format_offset.as_str()),
        (Some("156"), Some("208"), Some("20")),
        "the aligned DATA range and WEM-relative fmt chunk body position"
    );
    Ok(())
}

fn assert_schema(server: &mut Server, reply: &Value) -> TestResult {
    let schema = server
        .tools()?
        .into_iter()
        .find(|tool| tool.get("name").and_then(Value::as_str) == Some(TOOL))
        .ok_or("WEM info tool")?;
    jsonschema::validator_for(schema.get("outputSchema").ok_or("output schema")?)?
        .validate(reply)
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn assert_receipt(reply: &Value) -> TestResult {
    let origins = reply
        .get("fieldOrigins")
        .and_then(Value::as_object)
        .ok_or("field origins")?;
    for address in origins.keys() {
        let field = address
            .strip_prefix("/structuredContent")
            .ok_or("result field")?;
        assert!(
            reply.pointer(field).is_some(),
            "origin addresses emitted field {field}"
        );
        for site in uses(reply, address)? {
            physical_source(reply, site)?;
        }
    }
    for object in ["/wem", "/described"] {
        let data = reply.pointer(object).ok_or("WEM info object")?;
        assert_object_origins(reply, data, object)?;
    }
    let locations = reply
        .get("locations")
        .and_then(Value::as_array)
        .ok_or("location page")?;
    for (index, location) in locations.iter().enumerate() {
        assert_object_origins(reply, location, &format!("/locations/{index}"))?;
    }
    Ok(())
}

fn assert_object_origins(reply: &Value, object: &Value, path: &str) -> TestResult {
    for field in object.as_object().ok_or("WEM info fields")?.keys() {
        origin(reply, &format!("/structuredContent{path}/{field}"))?;
    }
    Ok(())
}
