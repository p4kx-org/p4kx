// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_testkit::{
    TestAnimation, TestChunkFile, TestDba, TestEntry, TestModel, TestSkeleton, TestTimes, TestTrack,
};
use serde_json::{Value, json};

use crate::model_animation_fixture::{DBA_SELECTED, DatabaseFixture, dba_fixture, set_end};
use crate::model_item_origin_checks::{assert_all_addresses, assert_site, field, marker, uses};
use crate::support::{Install, TestResult};

const SKIN: &str = "Data/Objects/Probe/rig.skin";
const SKIN_MESH: &str = "Data/Objects/Probe/rig.skinm";
const CAF: &str = "Data/Animations/Objects/Probe/extend.caf";
const DBA: &str = "Data/Animations/Objects/Probe/pack.dba";
const ANIM_INFO: u32 = 0x4733_C6ED;
const CAF_DATA: u32 = 0xA949_6CB5;
const RESPONSE_LIMIT: usize = 256 * 1024;

#[test]
fn caf_uses_selected_original_timing_and_only_binding_uses_the_encrypted_skeleton() -> TestResult {
    let animation = TestAnimation::new("extend.caf", 30)
        .track(TestTrack::new(7))
        .track(
            TestTrack::new(TestSkeleton::controller_id(1))
                .times(TestTimes::Shorts)
                .positions(&[(0, [0.0; 3]), (90, [1.0; 3])]),
        );
    let mut info = animation.anim_info();
    set_end(&mut info, 0, 15)?;
    let file = TestChunkFile::ivo()
        .chunk(0x1111, 0, [0])
        .chunk(ANIM_INFO, 0x901, info)
        .chunk(CAF_DATA, 0x900, animation.caf_data())
        .chunk(CAF_DATA, 0x900, [0]);
    let offsets = file.offsets();
    let info = *offsets.get(1).ok_or("selected AnimInfo offset")?;
    let data = *offsets.get(2).ok_or("selected CafData offset")?;
    let install = install_with(
        vec![
            TestEntry::zstd(CAF, file.bytes()),
            TestEntry::stored("data/animations/objects/probe/EXTEND.CAF", "losing entry")
                .encrypted(),
        ],
        true,
    )?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let response = server.call(
        "list_model_items",
        &json!({"path": SKIN, "section": "animations"}),
    )?;
    let reply = response
        .get("structuredContent")
        .ok_or("animation response")?;
    assert_caf_values(reply)?;
    assert_timing_sites(reply, info, data)?;
    assert!(
        uses(reply, "items/0/bound_tracks")?.iter().any(|site| {
            site.pointer("/location/namespace").and_then(Value::as_str) == Some("cryEngineSkeleton")
                && site.pointer("/location/field").and_then(Value::as_str) == Some("controllerIds")
        }),
        "the bound count uses the actual skeleton IDs"
    );
    assert!(
        uses(reply, "items/0/keys")?.iter().all(|site| {
            site.pointer("/location/namespace").and_then(Value::as_str) != Some("cryEngineSkeleton")
        }),
        "skeleton never contributes to the key count"
    );
    assert_eq!(
        reply
            .pointer("/provenance/sources")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(2),
        "only the selected CAF and model contribute"
    );
    assert_all_addresses(&response)
}

fn assert_caf_values(reply: &Value) -> TestResult {
    let row = reply.pointer("/items/0").ok_or("CAF row")?;
    for (name, expected) in [
        ("name", json!("extend")),
        ("path", json!("extend.caf")),
        ("source", json!(CAF)),
        ("frames_per_second", json!(30)),
        ("end_frame", json!(15)),
        ("duration_seconds", json!(3.0)),
        ("tracks", json!(1)),
        ("bound_tracks", json!(1)),
        ("keys", json!(2)),
        ("additive", json!(false)),
    ] {
        assert_eq!(row.get(name), Some(&expected), "existing CAF field {name}");
        marker(
            field(reply, &format!("items/0/{name}"))?,
            name == "bound_tracks",
        );
    }
    Ok(())
}

fn assert_timing_sites(reply: &Value, info: u64, data: u64) -> TestResult {
    assert_clip_span(reply, "end_frame", "end_frame", 0, 1, info + 16..info + 20)?;
    assert_clip_span(
        reply,
        "frames_per_second",
        "frames_per_second",
        0,
        1,
        info + 4..info + 6,
    )?;
    assert_clip_span(
        reply,
        "keys",
        "controllerDescriptors",
        0,
        2,
        data + 20..data + 68,
    )?;
    assert_frame(reply, "translation", 0, 2, 1, data + 70..data + 72)?;
    assert_clip_span(
        reply,
        "duration_seconds",
        "frameChannelDescriptor",
        0,
        2,
        data + 56..data + 68,
    )?;
    marker(field(reply, "total")?, false);
    marker(field(reply, "items")?, true);
    assert_site(
        reply,
        "items/0/keys",
        "cryEngineChunkTable",
        "animationTypePrefix",
        &["16", "20", "16", "3"],
    )?;
    Ok(())
}

#[test]
fn dba_keeps_the_original_clip_ordinal_and_a_bitmap_after_the_metadata_chunk() -> TestResult {
    let DatabaseFixture {
        bytes,
        name,
        bitmap,
    } = dba_fixture()?;
    let install = install_with(vec![TestEntry::zstd(DBA, bytes).encrypted()], false)?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let response = server.call(
        "list_model_items",
        &json!({"path": SKIN, "section": "animations"}),
    )?;
    let reply = response
        .get("structuredContent")
        .ok_or("animation response")?;
    assert_dba_values(reply)?;
    assert_clip_span(
        reply,
        "path",
        "databasePath",
        1,
        1,
        name as u64..(name + DBA_SELECTED.len() + 1) as u64,
    )?;
    assert_frame(
        reply,
        "rotation",
        1,
        0,
        0,
        bitmap as u64..(bitmap + 7) as u64,
    )?;
    assert_all_addresses(&response)?;
    let unread = server.ok(
        "list_model_items",
        &json!({"path": SKIN, "section": "unread_animations"}),
    )?;
    assert_eq!(
        unread.pointer("/items/0/name"),
        Some(&json!("unread")),
        "the rejected first clip is still unread"
    );
    assert_eq!(
        unread.get("total"),
        Some(&json!(1)),
        "unchanged unread count"
    );
    Ok(())
}

fn assert_dba_values(reply: &Value) -> TestResult {
    assert_eq!(
        reply.get("total"),
        Some(&json!(1)),
        "only the decoded clip is admitted"
    );
    assert_eq!(
        reply.pointer("/items/0/path"),
        Some(&json!(DBA_SELECTED)),
        "the second authored name"
    );
    assert_eq!(
        reply.pointer("/items/0/duration_seconds"),
        Some(&json!(1.0)),
        "decoded frame overrides the stored end"
    );
    marker(field(reply, "items/0/path")?, true);
    marker(field(reply, "items/0/source")?, false);
    Ok(())
}

#[test]
fn empty_tracks_use_no_skeleton_payload_and_a_stored_end_uses_no_key_frame() -> TestResult {
    let animation = TestAnimation::new("extend.caf", 10);
    let mut info = animation.anim_info();
    set_end(&mut info, 0, 30)?;
    let caf = TestChunkFile::ivo()
        .chunk(ANIM_INFO, 0x901, info)
        .chunk(CAF_DATA, 0x900, animation.caf_data())
        .bytes();
    let install = install_with(vec![TestEntry::stored(CAF, caf)], true)?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let reply = server.ok(
        "list_model_items",
        &json!({"path": SKIN, "section": "animations"}),
    )?;
    assert_eq!(
        reply.pointer("/items/0/duration_seconds"),
        Some(&json!(3.0)),
        "stored duration"
    );
    assert_eq!(
        reply.pointer("/items/0/bound_tracks"),
        Some(&json!(0)),
        "no track requires skeleton binding"
    );
    marker(field(&reply, "items/0/bound_tracks")?, false);
    for name in ["duration_seconds", "bound_tracks"] {
        assert!(
            uses(&reply, &format!("items/0/{name}"))?
                .iter()
                .all(|site| {
                    !matches!(
                        site.pointer("/location/namespace").and_then(Value::as_str),
                        Some("cryEngineAnimationFrame" | "cryEngineSkeleton")
                    )
                }),
            "{name} has no unused source"
        );
    }
    Ok(())
}

#[test]
fn animation_pages_fit_provenance_and_resume_at_the_last_returned_row() -> TestResult {
    let count = 220;
    let path = |index| format!("clips/{}_{}.caf", index, "x".repeat(420));
    let dba = (0..count).fold(TestDba::new(), |dba, index| {
        dba.animation(TestAnimation::new(&path(index), 30))
    });
    let install = install_with(vec![TestEntry::zstd(DBA, dba.bytes())], false)?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let mut args = json!({"path": SKIN, "section": "animations", "limit": 1000});
    let mut seen = 0;
    let mut pages = 0;
    loop {
        let response = server.call("list_model_items", &args)?;
        let reply = response.get("structuredContent").ok_or("animation page")?;
        let rows = reply
            .get("items")
            .and_then(Value::as_array)
            .ok_or("animation rows")?;
        assert_eq!(
            reply.get("total"),
            Some(&json!(count)),
            "complete row count"
        );
        assert!(
            serde_json::to_vec(reply)?.len() <= RESPONSE_LIMIT,
            "budget includes provenance"
        );
        assert!(!rows.is_empty(), "each cursor advances");
        assert_all_addresses(&response)?;
        for (position, row) in rows.iter().enumerate() {
            assert_eq!(
                row.get("path"),
                Some(&json!(path(seen))),
                "original clip order"
            );
            marker(field(reply, &format!("items/{position}/path"))?, false);
            seen += 1;
        }
        pages += 1;
        let Some(cursor) = reply.get("next_cursor") else {
            break;
        };
        args["cursor"] = cursor.clone();
    }
    assert!(pages > 1, "origin data forces multiple pages");
    assert_eq!(seen, count, "no missing or repeated rows");
    Ok(())
}

fn install_with(mut entries: Vec<TestEntry>, encrypted_skeleton: bool) -> TestResult<Install> {
    let skin = TestModel::two_bone_skin();
    let model = TestEntry::zstd(SKIN, skin.cgf());
    entries.push(if encrypted_skeleton {
        model.encrypted()
    } else {
        model
    });
    entries.push(TestEntry::zstd(SKIN_MESH, skin.cgfm()));
    Install::new(entries)
}

fn assert_clip_span(
    reply: &Value,
    output: &str,
    field: &str,
    clip: usize,
    chunk: usize,
    bytes: Range<u64>,
) -> TestResult {
    assert_site(
        reply,
        &format!("items/0/{output}"),
        "cryEngineAnimation",
        field,
        &[
            &clip.to_string(),
            &chunk.to_string(),
            &bytes.start.to_string(),
            &bytes.end.to_string(),
        ],
    )
}

fn assert_frame(
    reply: &Value,
    channel: &str,
    clip: usize,
    chunk: usize,
    controller: usize,
    bytes: Range<u64>,
) -> TestResult {
    assert_site(
        reply,
        "items/0/duration_seconds",
        "cryEngineAnimationFrame",
        channel,
        &[
            &clip.to_string(),
            &chunk.to_string(),
            &controller.to_string(),
            &bytes.start.to_string(),
            &bytes.end.to_string(),
        ],
    )
}
