// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::audio::AudioIndex;
use p4kx_extraction::{Archive, DataForgeEntry};
use p4kx_testkit::{TestAtl, TestEntry};
use serde_json::{Value, json};

use crate::exact_audio_assertions as check;
use crate::exact_audio_fixture::{self as fixture, Encoding, Texts};
use crate::exact_audio_source;
use crate::support::{Install, TestResult};

#[test]
fn exact_audio_worker_render_refusal_is_not_a_skipped_entity_lookup() -> TestResult {
    let mut texts = Texts::long();
    texts.trigger = format!("Exact_Worker_Huge_{}", "x".repeat(300 << 10));
    let install = fixture::install(&texts, Encoding::Text)?;
    exact_audio_source::original(&install, &texts)?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    check::too_large(
        &mut server,
        "search_audio_triggers",
        &json!({"search":"Exact_Worker_Huge","entities":1}),
    )
}

#[test]
fn exact_audio_worker_late_refusal_preserves_the_actual_prefix_and_cursor() -> TestResult {
    let mut texts = Texts::long();
    texts.later = format!("Exact_Worker_Late_{}", "z".repeat(300 << 10));
    let install = fixture::install(&texts, Encoding::CryXml)?;
    exact_audio_source::original(&install, &texts)?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let first = server.ok(
        "search_audio_triggers",
        &json!({"search":"Exact_","entities":1}),
    )?;
    assert_eq!(
        first
            .get("triggers")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(1),
        "only the actual complete original prefix is retained in the worker"
    );
    let hit = check::first(&first)?;
    assert_eq!(
        hit.get("name"),
        Some(&json!(texts.trigger)),
        "original first name"
    );
    assert_eq!(
        hit.pointer("/entities/count"),
        Some(&json!(1)),
        "actual first Entity ownership"
    );
    let cursor = first
        .get("next_cursor")
        .and_then(Value::as_str)
        .ok_or("exact continuation")?;
    check::too_large(
        &mut server,
        "search_audio_triggers",
        &json!({"search":"Exact_","entities":1,"cursor":cursor}),
    )
}

#[test]
fn exact_audio_worker_missing_dataforge_remains_an_explicit_lookup_skip() -> TestResult {
    let (install, original_error) = missing_dataforge_install()?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let empty = server.ok(
        "search_audio_triggers",
        &json!({"search":"Not_Present","entities":1}),
    )?;
    assert_eq!(
        empty.get("total"),
        Some(&json!(0)),
        "the actual query has no hit"
    );
    assert_eq!(
        empty.get("entities_skipped"),
        Some(&Value::Null),
        "no-hit response retains no lookup failure"
    );
    let matched = server.ok(
        "search_audio_triggers",
        &json!({"search":"Exact_NoForge","entities":1}),
    )?;
    assert_eq!(
        matched.get("total"),
        Some(&json!(1)),
        "the genuine audio definition remains"
    );
    assert_eq!(
        check::first(&matched)?.get("name"),
        Some(&json!("Exact_NoForge")),
        "original match"
    );
    let reason = matched
        .get("entities_skipped")
        .and_then(Value::as_str)
        .ok_or("explicit skipped reason")?;
    assert!(
        reason.contains(&original_error),
        "actual DataForge cause remains visible: {reason}"
    );
    assert_eq!(
        matched.get("unreadable_records"),
        Some(&json!(0)),
        "no Record walk was available"
    );
    Ok(())
}

fn missing_dataforge_install() -> TestResult<(Install, String)> {
    let bytes = TestAtl::new("OnlyAudio")
        .trigger("Exact_NoForge", &[])
        .to_xml()
        .into_bytes();
    let install = Install::new(vec![TestEntry::stored(
        "Data\\Libs\\GameAudio\\Only.xml",
        bytes,
    )])?;
    let archive = Archive::open(install.archive())?;
    let index = AudioIndex::build(&archive);
    assert!(
        index.issues().is_empty(),
        "the original ATL is readable independently of DataForge"
    );
    assert_eq!(index.triggers().len(), 1, "one original definition");
    assert_eq!(
        index.triggers().first().ok_or("original trigger")?.name(),
        "Exact_NoForge",
        "original name"
    );
    let Err(original_error) = DataForgeEntry::read(&archive) else {
        return Err("original Archive genuinely lacks DataForge".into());
    };
    let original_error = original_error.to_string();
    assert!(
        !original_error.is_empty(),
        "actual owning reader error before MCP"
    );
    Ok((install, original_error))
}
