// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Music segments, tracks, switch containers and playlists of the `HIRC` section, and the soundbank
//! a Play action names, decoded from synthetic banks.

mod decisions;
mod track;

use p4kx_audio::{BankError, HircKind, HircValue, PlayBank, SectionId, Soundbank};
use p4kx_testkit::{TestBank, TestHircObject};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const PARENT: u32 = 0x2C21_1B2E;

fn decoded(object: TestHircObject, version: u32) -> Result<(HircKind, HircValue), BankError> {
    let bytes = TestBank::new(1).version(version).object(object).bytes();
    let bank = Soundbank::parse(bytes.as_slice())?;
    Ok(bank
        .objects()
        .first()
        .map_or((HircKind::Other(0), HircValue::Raw), |object| {
            (object.kind, object.value.clone())
        }))
}

fn body_of(object: TestHircObject) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let bytes = TestBank::new(1).object(object).bytes();
    let bank = Soundbank::parse(bytes.as_slice())?;
    let object = bank.objects().first().ok_or("one object")?;
    Ok(bank
        .object_body(object)
        .get(4..)
        .unwrap_or_default()
        .to_vec())
}

#[test]
fn a_music_segment_is_decoded_with_its_tracks_meter_stingers_and_markers() -> TestResult {
    let object = TestHircObject::music_segment(5, &[30, 10], &["Entry", "Exit"]);
    let (kind, value) = decoded(object, 154)?;
    assert_eq!(kind, HircKind::MusicSegment);
    let HircValue::MusicSegment(segment) = value else {
        return Err(format!("{value:?}").into());
    };
    assert_eq!(segment.node.parent_id, PARENT, "parent");
    assert_eq!(segment.node.children, [30, 10], "tracks in bank order");
    assert_eq!(
        segment.node.meter.tempo.to_bits(),
        120.0_f32.to_bits(),
        "tempo"
    );
    assert_eq!(
        (
            segment.node.meter.beats_per_bar,
            segment.node.meter.beat_value
        ),
        (4, 4),
        "time signature"
    );
    assert_eq!(
        segment
            .node
            .stingers
            .iter()
            .map(|stinger| (stinger.trigger_id, stinger.segment_id))
            .collect::<Vec<_>>(),
        [(0x5100_0001, 0x5100_0002)],
        "stinger"
    );
    assert_eq!(segment.duration.to_bits(), 4000.5_f64.to_bits(), "duration");
    assert_eq!(
        segment
            .markers
            .iter()
            .map(|marker| (marker.id, &*marker.name))
            .collect::<Vec<_>>(),
        [(0x100, "Entry"), (0x101, "Exit")],
        "marker names"
    );
    Ok(())
}

#[test]
fn a_music_switch_container_lists_its_children_and_the_targets_of_its_decision_tree() -> TestResult
{
    let object = TestHircObject::music_switch(7, &[40, 41, 42], 0x4000, &[41, 0, 42]);
    let HircValue::MusicSwitch(switch) = decoded(object, 154)?.1 else {
        return Err("a music switch".into());
    };
    assert_eq!(switch.node.children, [40, 41, 42], "children");
    assert_eq!(switch.rule_count, 2, "transition rules");
    assert_eq!(switch.group_ids, [0x4000], "groups");
    assert_eq!(switch.targets, [41, 42], "leaves, 0 left out");
    assert!(
        switch.has_decision_conditions,
        "nonempty original graph and arguments"
    );
    Ok(())
}

#[test]
fn an_empty_music_switch_tree_does_not_apply_decision_conditions() -> TestResult {
    let object = TestHircObject::music_switch_with_tree(7, &[], &[(0x4000, 0)], &[]);
    let HircValue::MusicSwitch(switch) = decoded(object, 154)?.1 else {
        return Err("checked empty MusicSwitch".into());
    };
    assert!(
        !switch.has_decision_conditions,
        "an argument without any node is insufficient"
    );
    Ok(())
}

#[test]
fn a_music_playlist_keeps_its_items_with_their_loop_counts() -> TestResult {
    let object = TestHircObject::music_playlist(8, &[40, 41]);
    let HircValue::MusicPlaylist(playlist) = decoded(object, 154)?.1 else {
        return Err("a music playlist".into());
    };
    assert_eq!(playlist.node.children, [40, 41], "segments");
    assert_eq!(playlist.rule_count, 2, "transition rules");
    assert!(
        playlist.has_authored_loops,
        "original item count and modifiers"
    );
    assert_eq!(
        playlist
            .items
            .iter()
            .map(|item| (
                item.segment_id,
                item.item_id,
                item.loops,
                item.loop_min,
                item.loop_max
            ))
            .collect::<Vec<_>>(),
        [(40, 1, -1, 2, 5), (41, 2, -1, 2, 5)],
        "items"
    );
    assert!(
        playlist
            .items
            .iter()
            .all(|item| item.uses_weight && !item.shuffle)
    );
    Ok(())
}

#[test]
fn music_objects_of_other_bank_versions_are_not_decoded() -> TestResult {
    for (version, expected_raw) in [(150, true), (155, false)] {
        let object = TestHircObject::music_segment(5, &[1], &[]);
        let (_, value) = decoded(object, version)?;
        if expected_raw {
            assert_eq!(value, HircValue::Raw, "version {version}");
        } else {
            assert!(
                matches!(
                    value,
                    HircValue::Malformed(BankError::UnsupportedObjectLayout { version: 155, .. })
                ),
                "version {version}: {value:?}"
            );
        }
    }
    Ok(())
}

#[test]
fn a_truncated_or_overlong_music_object_is_malformed_and_the_bank_still_parses() -> TestResult {
    for (type_byte, object) in [
        (10, TestHircObject::music_segment(5, &[1, 2], &["A"])),
        (11, TestHircObject::music_switch_track(5, &[9], 3, &[4])),
        (12, TestHircObject::music_switch(5, &[1], 3, &[1])),
        (13, TestHircObject::music_playlist(5, &[1, 2])),
    ] {
        let body = body_of(object)?;
        for (name, cut) in [
            (
                "truncated",
                body.get(..body.len() - 2).unwrap_or_default().to_vec(),
            ),
            ("overlong", [body.as_slice(), &[0]].concat()),
            (
                "half",
                body.get(..body.len() / 2).unwrap_or_default().to_vec(),
            ),
        ] {
            let (kind, value) = decoded(TestHircObject::raw(type_byte, 5, cut), 154)?;
            assert_eq!(kind, HircKind::from_byte(type_byte), "{name}: kind kept");
            assert!(
                matches!(
                    value,
                    HircValue::Malformed(BankError::Truncated { section, .. }
                        | BankError::InvalidSection { section, .. }) if section == SectionId::HIRC
                ),
                "type {type_byte} {name}: {value:?}"
            );
        }
    }
    Ok(())
}

#[test]
fn a_decision_tree_that_lies_outside_the_tree_is_malformed() -> TestResult {
    let object = TestHircObject::music_switch(7, &[40], 0x4000, &[40]);
    let body = body_of(object)?;
    let tree_start = body.len() - 24;
    for (name, patch) in [
        ("first child outside", (tree_start + 4, [50, 0])),
        ("more children than nodes", (tree_start + 6, [9, 0])),
    ] {
        let mut cut = body.clone();
        let at = patch.0;
        cut.splice(at..at + 2, patch.1);
        let (_, value) = decoded(TestHircObject::raw(12, 7, cut), 154)?;
        assert!(
            matches!(
                value,
                HircValue::Malformed(BankError::InvalidSection { .. })
            ),
            "{name}: {value:?}"
        );
    }
    Ok(())
}

#[test]
fn an_ancestor_cycle_or_out_of_bounds_children_are_malformed() -> TestResult {
    for (name, targets, first, count) in [
        (
            "a child interval spilling past its only leaf",
            vec![40],
            1_u16,
            2_u16,
        ),
        ("the root listed as its own child", vec![40, 41, 42], 0, 4),
        (
            "a child interval one past the node extent",
            vec![40, 41, 42],
            1,
            4,
        ),
    ] {
        let body = body_of(TestHircObject::music_switch(7, &[40], 0x4000, &targets))?;
        let tree_start = body.len() - 12 * (targets.len() + 1);
        let mut cut = body;
        cut.splice(tree_start + 4..tree_start + 6, first.to_le_bytes());
        cut.splice(tree_start + 6..tree_start + 8, count.to_le_bytes());
        let (_, value) = decoded(TestHircObject::raw(12, 7, cut), 154)?;
        assert!(
            matches!(
                value,
                HircValue::Malformed(BankError::InvalidSection { .. })
            ),
            "{name}: {value:?}"
        );
    }
    Ok(())
}

#[test]
fn counts_beyond_the_body_are_malformed_without_allocating_them() -> TestResult {
    let object = TestHircObject::music_playlist(8, &[40]);
    let body = body_of(object)?;
    let items = body.len() - 30 - 4;
    let mut huge_items = body;
    huge_items.splice(items..items + 4, u32::MAX.to_le_bytes());
    let (_, value) = decoded(TestHircObject::raw(13, 8, huge_items), 154)?;
    assert!(matches!(value, HircValue::Malformed(_)), "{value:?}");

    let segment = body_of(TestHircObject::music_segment(5, &[1], &[]))?;
    let markers = segment.len() - 4;
    let mut huge_markers = segment;
    huge_markers.splice(markers..markers + 4, u32::MAX.to_le_bytes());
    let (_, value) = decoded(TestHircObject::raw(10, 5, huge_markers), 154)?;
    assert!(matches!(value, HircValue::Malformed(_)), "{value:?}");
    Ok(())
}

#[test]
fn a_play_action_names_its_soundbank() -> TestResult {
    let object = TestHircObject::action_in_bank(9, 0x0403, 77, 0xB4A1_0C03);
    let (_, value) = decoded(object, 154)?;
    let HircValue::Action(action) = value else {
        return Err("an action".into());
    };
    assert_eq!(
        (action.target_id, action.bank),
        (
            77,
            Some(PlayBank {
                bank_id: 0xB4A1_0C03,
                bank_type: 0
            })
        )
    );
    assert!(action.is_play() && !action.is_play_event() && !action.is_trigger());
    Ok(())
}

#[test]
fn only_a_play_action_of_version_154_reads_a_soundbank() -> TestResult {
    let (_, stop) = decoded(TestHircObject::action(9, 0x0103, 77), 154)?;
    assert!(
        matches!(&stop, HircValue::Action(action) if action.bank.is_none()),
        "{stop:?}"
    );
    let (_, old) = decoded(TestHircObject::action(9, 0x0403, 77), 150)?;
    assert!(
        matches!(&old, HircValue::Action(action) if action.bank.is_none()),
        "{old:?}"
    );
    let (_, play_event) = decoded(TestHircObject::action(9, 0x2103, 77), 154)?;
    assert!(
        matches!(&play_event, HircValue::Action(action) if action.is_play_event()),
        "{play_event:?}"
    );
    let (_, trigger) = decoded(TestHircObject::action(9, 0x1B02, 77), 154)?;
    assert!(
        matches!(&trigger, HircValue::Action(action) if action.is_trigger()),
        "{trigger:?}"
    );
    Ok(())
}

#[test]
fn a_play_action_with_bytes_after_its_fields_is_malformed() -> TestResult {
    let mut body = body_of(TestHircObject::action_in_bank(9, 0x0403, 77, 1))?;
    body.push(0);
    let (_, value) = decoded(TestHircObject::raw(3, 9, body), 154)?;
    assert!(matches!(value, HircValue::Malformed(_)), "{value:?}");
    Ok(())
}

#[test]
fn a_sound_reads_its_parent_and_a_sound_with_bytes_after_its_node_base_is_malformed() -> TestResult
{
    let (_, value) = decoded(TestHircObject::sound(3, 4, 0, 5), 154)?;
    assert!(
        matches!(&value, HircValue::Sound(source) if source.parent_id == Some(PARENT)),
        "{value:?}"
    );
    let mut body = body_of(TestHircObject::sound(3, 4, 0, 5))?;
    body.push(0);
    let (_, value) = decoded(TestHircObject::raw(2, 3, body), 154)?;
    assert!(matches!(value, HircValue::Malformed(_)), "{value:?}");
    Ok(())
}
