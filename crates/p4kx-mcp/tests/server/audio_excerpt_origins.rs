// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{TestBank, TestEntry};
use serde_json::{Value, json};

use crate::audio::{serve, stereo};
use crate::support::{Install, TestResult};

const WAV: &str = "/content/1/data";

#[test]
fn an_encrypted_loose_excerpt_binds_the_exact_pcm_window_and_leaves_the_path_plain() -> TestResult {
    let path = "Data/Sounds/wwise/Media/7300.wem";
    let install = Install::new(vec![TestEntry::deflated(path, stereo(8000, 1)).encrypted()])?;
    let mut server = serve(&install)?;
    let reply = server.call(
        "get_audio_excerpt",
        &json!({
            "path": path,
            "start_seconds": 0.25,
            "duration_seconds": 0.25
        }),
    )?;
    let meta = reply.get("structuredContent").ok_or("structured excerpt")?;
    assert_eq!(meta.get("frames"), Some(&json!(2000)));
    assert!(reply.pointer(WAV).and_then(Value::as_str).is_some());
    assert_marker(meta, WAV, true)?;
    assert_marker(meta, "/structuredContent/codec", true)?;
    assert_marker(meta, "/structuredContent/source/path", false)?;
    assert_marker(meta, "/structuredContent/wem_id", false)?;

    let pcm = uses(meta, WAV)?
        .into_iter()
        .find(|site| site.pointer("/location/namespace") == Some(&json!("wemDecodedEntryBytes")))
        .ok_or("original PCM sample window")?;
    assert_eq!(
        pcm.pointer("/location/indices"),
        Some(&json!(["8044", "16044"]))
    );
    assert_eq!(source_path(meta, pcm)?, path);
    Ok(())
}

#[test]
fn a_case_alias_with_another_wem_cannot_replace_the_selected_encrypted_bank() -> TestResult {
    let other = "Data/Sounds/wwise/Voice.bnk";
    let selected = "Data/Sounds/wwise/voice.bnk";
    let bank = TestBank::new(7).wem(10, stereo(8000, 1)).bytes();
    let encrypted = TestBank::new(8)
        .wem(1, stereo(8000, 1))
        .wem(7301, stereo(16000, 1))
        .bytes();
    let install = Install::new(vec![
        TestEntry::stored(other, bank),
        TestEntry::deflated(selected, encrypted).encrypted(),
    ])?;
    let mut server = serve(&install)?;
    let reply = server.call(
        "get_audio_excerpt",
        &json!({"wem_id": 7301, "duration_seconds": 0.25}),
    )?;
    let meta = reply.get("structuredContent").ok_or("structured excerpt")?;
    assert_eq!(meta.get("sample_rate"), Some(&json!(16000)));
    assert_eq!(meta.pointer("/source/path"), Some(&json!(selected)));
    assert_marker(meta, WAV, true)?;
    assert_marker(meta, "/structuredContent/source/path", false)?;
    assert_marker(meta, "/structuredContent/source/len", true)?;
    assert_marker(meta, "/structuredContent/source/kind", true)?;
    assert_marker(meta, "/structuredContent/source/offset", true)?;
    assert_marker(meta, "/structuredContent/wem_id", false)?;
    let offset = meta
        .pointer("/source/offset")
        .and_then(Value::as_u64)
        .ok_or("embedded offset")?;
    assert!(offset > 0, "the WEM follows another media item");
    let pcm = uses(meta, WAV)?
        .into_iter()
        .find(|site| {
            site.pointer("/location/namespace")
                == Some(&json!("bnkEmbeddedWem.wemDecodedEntryBytes"))
        })
        .ok_or("embedded PCM window")?;
    assert_eq!(pcm.pointer("/location/indices/0"), Some(&json!("7301")));
    assert_eq!(
        pcm.pointer("/location/indices/1"),
        Some(&json!(offset.to_string()))
    );
    assert_eq!(source_path(meta, pcm)?, selected);
    Ok(())
}

#[test]
fn a_loose_localized_excerpt_traces_the_entry_that_supplied_the_display_spelling() -> TestResult {
    let spelling = "Data/Sounds/wwise/English/Language.bnk";
    let sound = "Data/Sounds/wwise/ENGLISH/Media/7302.wem";
    let install = Install::new(vec![
        TestEntry::stored(spelling, TestBank::new(1).bytes()),
        TestEntry::deflated(sound, stereo(8000, 1)).encrypted(),
    ])?;
    let mut server = serve(&install)?;
    let reply = server.call(
        "get_audio_excerpt",
        &json!({"wem_id": 7302, "duration_seconds": 0.25}),
    )?;
    let meta = reply.get("structuredContent").ok_or("structured excerpt")?;
    assert_eq!(meta.pointer("/source/language"), Some(&json!("English")));
    assert_marker(meta, "/structuredContent/source/language", false)?;
    let paths = uses(meta, "/structuredContent/source/language")?
        .into_iter()
        .map(|site| source_path(meta, site))
        .collect::<TestResult<Vec<_>>>()?;
    assert!(paths.contains(&spelling));
    assert!(paths.contains(&sound));
    for site in uses(meta, WAV)? {
        assert_eq!(source_path(meta, site)?, sound);
    }
    Ok(())
}

#[test]
fn the_excerpt_schema_declares_the_binary_origin_and_all_pointers_resolve() -> TestResult {
    let path = "Data/Sounds/wwise/Media/7303.wem";
    let install = Install::new(vec![TestEntry::stored(path, stereo(8000, 1))])?;
    let mut server = serve(&install)?;
    let tools = server.tools()?;
    let tool = tools
        .iter()
        .find(|item| item.get("name") == Some(&json!("get_audio_excerpt")))
        .ok_or("excerpt tool")?;
    let fields = tool
        .pointer("/outputSchema/$defs/ExcerptFieldOrigins")
        .ok_or("field schema")?;
    assert_eq!(fields.get("additionalProperties"), Some(&json!(false)));
    let required = fields
        .get("required")
        .and_then(Value::as_array)
        .ok_or("required origins")?;
    assert!(required.contains(&json!(WAV)));
    assert!(!required.contains(&json!("/structuredContent/source/offset")));

    let reply = server.call(
        "get_audio_excerpt",
        &json!({"path": path, "duration_seconds": 0.25}),
    )?;
    let origins = reply
        .pointer("/structuredContent/fieldOrigins")
        .and_then(Value::as_object)
        .ok_or("field origins")?;
    for pointer in origins.keys() {
        assert!(reply.pointer(pointer).is_some(), "{pointer}");
    }
    Ok(())
}

fn origin<'a>(meta: &'a Value, field: &str) -> TestResult<&'a Value> {
    meta.get("fieldOrigins")
        .and_then(|fields| fields.get(field))
        .ok_or_else(|| format!("missing origin: {field}").into())
}

fn assert_marker(meta: &Value, field: &str, encrypted: bool) -> TestResult {
    let origin = origin(meta, field)?;
    assert_eq!(
        origin.get("containsEncryptedSource"),
        Some(&json!(encrypted)),
        "{field}"
    );
    assert_eq!(
        origin.get("sourceSetComplete"),
        Some(&json!(true)),
        "{field}"
    );
    Ok(())
}

fn uses<'a>(meta: &'a Value, field: &str) -> TestResult<Vec<&'a Value>> {
    let refs = origin(meta, field)?
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("source references")?;
    let uses = meta
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("source uses")?;
    refs.iter()
        .map(|id| {
            uses.iter()
                .find(|site| site.get("id") == Some(id))
                .ok_or_else(|| "unresolved use".into())
        })
        .collect()
}

fn source_path<'a>(meta: &'a Value, site: &Value) -> TestResult<&'a str> {
    meta.pointer("/provenance/sources")
        .and_then(Value::as_array)
        .and_then(|sources| {
            sources
                .iter()
                .find(|source| source.get("id") == site.get("source"))
        })
        .and_then(|source| source.pointer("/path/value"))
        .and_then(Value::as_str)
        .ok_or_else(|| "source path".into())
}
