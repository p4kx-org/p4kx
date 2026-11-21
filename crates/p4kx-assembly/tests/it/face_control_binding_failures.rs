// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_assembly::ArchiveModels;
use p4kx_model::Animation;
use p4kx_testkit::{TestAnimation, TestTrack};
use serde_json::{Value, json};

use super::{TestResult, animation, fixture, track};

fn report(clip: &Animation) -> Result<Value, Box<dyn Error>> {
    let npc = fixture()?;
    let models = ArchiveModels::new(&npc.archive);
    let error = npc
        .armour
        .verify_face_rotations(clip, &models, &[0])
        .err()
        .ok_or("the incomplete control binding must remain refused")?;
    let (_, detail) = error
        .split_once("; controllerBinding=")
        .ok_or("the failure must carry the selected control evidence")?;
    Ok(serde_json::from_str(detail)?)
}

fn row<'a>(report: &'a Value, name: &str) -> Result<&'a Value, Box<dyn Error>> {
    let id = crc32fast::hash(name.as_bytes());
    let rows = report
        .get("tracks")
        .and_then(Value::as_array)
        .ok_or("complete track evidence")?;
    let mut found = rows
        .iter()
        .filter(|row| row.get("controllerId").and_then(Value::as_u64) == Some(u64::from(id)));
    let selected = found.next().ok_or("the selected controller")?;
    assert!(found.next().is_none(), "the synthetic control is unique");
    Ok(selected)
}

#[test]
fn failure_separates_unknown_missing_translation_and_constant_controls() -> TestResult {
    let clip = animation(
        &TestAnimation::new("face.caf", 30)
            .track(track("CTL_PSD"))
            .track(
                TestTrack::new(crc32fast::hash(b"CTL_a"))
                    .rotations(&[(0, [0.0, 0.0, 0.0, 1.0]), (20, [0.0, 0.0, 0.0, 1.0])]),
            )
            .track(track("CTL_b"))
            .track(
                TestTrack::new(crc32fast::hash(b"unmapped-zero"))
                    .positions(&[(0, [0.0; 3]), (20, [0.0; 3])]),
            )
            .track(track("unmapped-moving")),
    )?;
    let detail = report(&clip)?;
    assert_eq!(
        detail.get("groups"),
        Some(&json!({"usableControl":1, "constantControl":1,
            "missingTranslation":1, "unknownController":2})),
        "authored controls and missing channels have separate failure groups"
    );
    assert_eq!(
        detail.get("selection"),
        Some(&json!({"usableControls":1,"drivenTracks":2,"skippedTracks":3})),
        "the report retains the actual strict selection counts"
    );
    assert_context(&detail);
    assert_track_details(&detail, &clip)?;
    Ok(())
}

fn assert_context(detail: &Value) {
    assert_eq!(
        detail.get("rigSource"),
        Some(&json!({"kind":"own","path":null})),
        "an unavailable own-rig path is not reconstructed"
    );
    assert_eq!(
        detail.pointer("/part/name"),
        Some(&json!("head")),
        "selected facial part"
    );
}

fn assert_track_details(detail: &Value, clip: &Animation) -> TestResult {
    let known = row(detail, "CTL_a")?;
    assert_eq!(
        known.get("group"),
        Some(&json!("missingTranslation")),
        "known hash without values"
    );
    assert_eq!(
        known.get("controlName"),
        Some(&json!("CTL_a")),
        "actual rig control name"
    );
    assert_eq!(
        known.pointer("/translation/present"),
        Some(&json!(false)),
        "missing translation channel"
    );
    let zero = row(detail, "unmapped-zero")?;
    assert_eq!(
        zero.get("group"),
        Some(&json!("unknownController")),
        "zero keys cannot authorize an unknown controller"
    );
    assert_eq!(
        zero.pointer("/translation/firstChannelActivity"),
        Some(&json!("zero")),
        "raw zero activity is evidence only"
    );
    let moving = row(detail, "unmapped-moving")?;
    assert_eq!(
        moving.pointer("/translation/firstChannelActivity"),
        Some(&json!("varying")),
        "unmapped activity is retained"
    );
    let tracks = detail
        .get("tracks")
        .and_then(Value::as_array)
        .ok_or("track evidence")?;
    assert_eq!(
        tracks.len(),
        clip.tracks.len(),
        "no failed track is truncated"
    );
    for (index, (actual, original)) in tracks.iter().zip(&clip.tracks).enumerate() {
        assert_eq!(
            actual.get("track"),
            Some(&json!(index)),
            "actual decoded position"
        );
        assert_eq!(
            actual.get("controllerId"),
            Some(&json!(original.controller_id)),
            "original controller identity"
        );
    }
    Ok(())
}

#[test]
fn a_constant_control_alone_does_not_become_a_complete_facial_sequence() -> TestResult {
    let clip = animation(&TestAnimation::new("face.caf", 30).track(track("CTL_PSD")))?;
    let detail = report(&clip)?;
    assert_eq!(
        detail.get("selection"),
        Some(&json!({"usableControls":0,"drivenTracks":1,"skippedTracks":0})),
        "the absence of usable controls still refuses the comparison"
    );
    assert_eq!(
        row(&detail, "CTL_PSD")?.get("group"),
        Some(&json!("constantControl")),
        "constant rig slot zero"
    );
    Ok(())
}

#[test]
fn an_empty_translation_channel_remains_distinct_from_a_missing_channel() -> TestResult {
    let mut clip = animation(
        &TestAnimation::new("face.caf", 30)
            .track(
                track("CTL_a").rotations(&[(0, [0.0, 0.0, 0.0, 1.0]), (20, [0.0, 0.0, 0.0, 1.0])]),
            )
            .track(track("unmapped-moving")),
    )?;
    let known = clip
        .tracks
        .iter_mut()
        .find(|track| track.controller_id == crc32fast::hash(b"CTL_a"))
        .ok_or("known control")?;
    let translation = known
        .translations
        .as_mut()
        .ok_or("present source channel")?;
    translation.frames.clear();
    translation.values.clear();
    let detail = report(&clip)?;
    let known = row(&detail, "CTL_a")?;
    assert_eq!(
        known.get("group"),
        Some(&json!("usableControl")),
        "the diagnostic preserves actual channel-presence selection"
    );
    assert_eq!(
        known.get("translation"),
        Some(&json!({"present":true,"frames":0,"values":0,"firstChannelActivity":"empty"})),
        "empty is not missing"
    );
    assert_eq!(
        detail.pointer("/groups/missingTranslation"),
        Some(&json!(0)),
        "only the unknown hash causes skipping"
    );
    Ok(())
}

#[test]
fn repeated_controller_occurrences_keep_their_actual_track_positions() -> TestResult {
    let mut clip = animation(&TestAnimation::new("face.caf", 30).track(track("unmapped-moving")))?;
    let repeat = clip.tracks.first().ok_or("one synthetic track")?.clone();
    clip.tracks.push(repeat);
    let detail = report(&clip)?;
    let tracks = detail
        .get("tracks")
        .and_then(Value::as_array)
        .ok_or("track evidence")?;
    assert_eq!(tracks.len(), 2, "both authored occurrences are retained");
    assert_eq!(
        detail.pointer("/groups/unknownController"),
        Some(&json!(2)),
        "same controller identity does not collapse occurrences"
    );
    assert_eq!(
        tracks.first().and_then(|row| row.get("track")),
        Some(&json!(0)),
        "first decoded track"
    );
    assert_eq!(
        tracks.get(1).and_then(|row| row.get("track")),
        Some(&json!(1)),
        "second decoded track"
    );
    Ok(())
}

#[test]
fn nonfinite_unknown_values_remain_explicit_in_valid_diagnostic_json() -> TestResult {
    let mut clip = animation(&TestAnimation::new("face.caf", 30).track(track("unmapped-moving")))?;
    let value = clip
        .tracks
        .first_mut()
        .and_then(|track| track.translations.as_mut())
        .and_then(|keys| keys.values.first_mut())
        .ok_or("one original value")?;
    let [first, ..] = value;
    *first = f32::NAN;
    let detail = report(&clip)?;
    assert_eq!(
        row(&detail, "unmapped-moving")?.pointer("/translation/firstChannelActivity"),
        Some(&json!("nonFinite")),
        "invalid numeric evidence cannot silently become JSON null"
    );
    Ok(())
}
