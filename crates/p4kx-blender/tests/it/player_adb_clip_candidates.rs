// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::BodyVariant;
use p4kx_testkit::{CryXmlByteOrder, CryXmlSignature, TestElement, TestEntry};
use serde_json::{Value, json};

use super::rig_clip_candidates::{self as fixture, family};
use super::{TestResult, fixture_with_motion, motion_option_assertions};

#[test]
fn authored_adb_aliases_share_physical_alternatives_between_exact_occurrences() -> TestResult {
    let rows = capture()?;
    let groups = family(&rows, "adbClipAliasGroup");
    assert_groups(&groups)?;
    let occurrences = family(&rows, "adbClipAliasOccurrence");
    assert_occurrences(&occurrences, &rows)?;
    let memberships = family(&rows, "rigClipAliasCandidate");
    assert_candidates(&memberships, &rows)
}

fn capture() -> TestResult<Vec<Value>> {
    let [controller, _, fragment_defs] = motion_option_assertions::entries("");
    let layer = TestElement::new("AnimLayer")
        .child(TestElement::new("Blend"))
        .child(TestElement::new("Animation").attribute("name", "Shared"))
        .child(TestElement::new("Animation").attribute("name", "Missing"))
        .child(TestElement::new("Other"))
        .child(TestElement::new("Animation").attribute("name", "sHaReD"))
        .child(TestElement::new("Animation").attribute("name", "unsupported/path"))
        .child(
            TestElement::new("Animation")
                .attribute("name", "Shared")
                .attribute("name", "Loose"),
        )
        .child(TestElement::new("Animation"));
    let fragment = TestElement::new("Fragment")
        .child(TestElement::new("ProcLayer"))
        .child(layer)
        .child(
            TestElement::new("AnimLayer")
                .child(TestElement::new("Animation").attribute("name", "Loose")),
        );
    let adb = TestElement::new("AnimDB")
        .attribute("FragDef", "Animations/Test/fragments.xml")
        .child(TestElement::new("FragmentList").child(TestElement::new("Idle").child(fragment)))
        .to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little);
    let extras = fixture::body_entries(BodyVariant::Male)?
        .into_iter()
        .chain(fixture::body_entries(BodyVariant::Female)?)
        .chain(fixture::animation_entries()?);
    let (_original, archive) = fixture_with_motion(
        [
            controller,
            TestEntry::stored("Data/Animations/test/player.adb", adb).encrypted(),
            fragment_defs,
        ],
        extras,
    )?;
    fixture::capture_rows(&archive)
}

fn assert_groups(groups: &[&Value]) -> TestResult {
    let [shared, missing, loose] = groups else {
        return Err("expected one group per authored key".into());
    };
    for (index, (row, alias)) in [
        (*shared, "shared"),
        (*missing, "missing"),
        (*loose, "loose"),
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(
            row.get("index"),
            Some(&json!(index)),
            "first authored order"
        );
        assert_eq!(
            row.pointer("/value/aliasKey/value"),
            Some(&json!(alias)),
            "normalized authored alias"
        );
        assert_eq!(
            row.pointer("/value/aliasKey/containsEncryptedSource"),
            Some(&json!(true)),
            "authored alias comes from encrypted ADB"
        );
        assert_eq!(
            row.pointer("/value/controllerRigBinding"),
            Some(&json!("unresolved")),
            "shared alias is a candidate relation"
        );
    }
    Ok(())
}

fn assert_occurrences(occurrences: &[&Value], rows: &[Value]) -> TestResult {
    let [
        first,
        missing,
        repeat,
        unsupported,
        ambiguous,
        absent,
        loose,
    ] = occurrences
    else {
        return Err("expected every authored animation occurrence".into());
    };
    for (row, layer, animation, state, group) in [
        (*first, 1, 1, "present", Some(0)),
        (*missing, 1, 2, "present", Some(1)),
        (*repeat, 1, 4, "present", Some(0)),
        (*unsupported, 1, 5, "unsupported", None),
        (*ambiguous, 1, 6, "invalid", None),
        (*absent, 1, 7, "absent", None),
        (*loose, 2, 0, "present", Some(2)),
    ] {
        assert_eq!(
            row.pointer("/value/layer/value"),
            Some(&json!(layer)),
            "original layer coordinate"
        );
        assert_eq!(
            row.pointer("/value/animation/value"),
            Some(&json!(animation)),
            "original child coordinate"
        );
        assert_eq!(
            row.pointer("/value/group/state"),
            Some(&json!(state)),
            "explicit key disposition"
        );
        assert_eq!(
            row.pointer("/value/group/value"),
            Some(&json!(group)),
            "link to established alias group"
        );
        assert!(
            family(rows, "adbFragment").iter().any(|fragment| {
                fragment.get("document") == row.pointer("/value/document")
                    && fragment.get("index") == row.pointer("/value/fragment")
            }),
            "occurrence resolves to the original fragment"
        );
        assert_eq!(
            row.pointer("/value/group/containsEncryptedSource"),
            Some(&json!(true)),
            "ADB declarations own their individual links"
        );
    }
    assert_ne!(
        first.pointer("/value/animation/sourceUsesRefs"),
        repeat.pointer("/value/animation/sourceUsesRefs"),
        "same normalized alias preserves submitted occurrences"
    );
    Ok(())
}

fn assert_candidates(memberships: &[&Value], rows: &[Value]) -> TestResult {
    let coordinates = memberships
        .iter()
        .map(|row| {
            (
                row.pointer("/value/rig").and_then(Value::as_u64),
                row.pointer("/value/record").and_then(Value::as_u64),
                row.pointer("/value/group/value").and_then(Value::as_u64),
            )
        })
        .collect::<Vec<_>>();
    let expected = [
        (0, 0, 0),
        (1, 0, 0),
        (0, 1, 0),
        (1, 1, 0),
        (0, 2, 0),
        (1, 2, 0),
        (0, 3, 2),
        (1, 3, 2),
    ]
    .map(|(rig, record, group)| (Some(rig), Some(record), Some(group)));
    assert_eq!(
        coordinates, expected,
        "one membership per physical clip regardless of repeated authored aliases"
    );
    let clips = family(rows, "rigClipCandidate");
    for item in memberships {
        assert_membership(item, &clips)?;
    }
    Ok(())
}

fn assert_membership(item: &Value, clips: &[&Value]) -> TestResult {
    let relation = item.get("value").ok_or("missing candidate link")?;
    let clip = clips
        .iter()
        .find(|clip| {
            clip.get("rig") == relation.get("rig")
                && ["rule", "chunk", "record"].iter().all(|key| {
                    clip.get("value").and_then(|value| value.get(key)) == relation.get(key)
                })
        })
        .ok_or("missing exact physical candidate")?;
    assert_eq!(
        clip.pointer("/value/path/containsEncryptedSource"),
        Some(&json!(false)),
        "intrinsic physical path stays clear"
    );
    assert_eq!(
        item.pointer("/value/group/containsEncryptedSource"),
        Some(&json!(true)),
        "accepted candidate retains the encrypted authored selector"
    );
    assert!(
        item.pointer("/value/group/sourceUsesRefs")
            .and_then(Value::as_array)
            .is_some_and(|uses| uses.len() >= 2),
        "joined alias keeps evidence from both sides"
    );
    Ok(())
}
