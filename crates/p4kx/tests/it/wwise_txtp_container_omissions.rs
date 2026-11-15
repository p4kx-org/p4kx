// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;

use p4kx_testkit::{
    ArchiveBuilder, TestAtl, TestBank, TestContainerKind, TestEntry, TestHircObject, TestNodeBase,
    wwise_short_id,
};
use serde_json::json;

use super::txtp_origin_assertions::check_plain_fields;
use super::{EMBEDDED, Fixture, PLAY, TestResult, extract, hirc_payload, pcm};

const REPEAT: &str = "Play_Repeat_Container";
const INFINITE: &str = "Play_Infinite_Container";
const RANGE: &str = "Play_Loop_Range";
const SWITCH: &str = "Play_Conditioned_Container";
const DIRECT: &str = "Play_Same_Sound_Directly";
const DELAY: &str = "Play_Sequence_Delay";
const INACTIVE_DELAY: &str = "Play_Sequence_Disabled_Transition";
const BLEND: &str = "Play_Blend_Curve";
const EMPTY_BLEND: &str = "Play_Empty_Blend_Curve";
const FLAGS: [&str; 3] = ["--format", "wav", "--txtp"];
const LOOPS: &str = "omit-container-loops";
const CONDITIONS: &str = "omit-container-conditions";
const SOUND: u32 = 20;
const SECOND_SOUND: u32 = 21;
const CHILDREN: [u32; 2] = [SOUND, SECOND_SOUND];
const WEM: u32 = 100;
const SECOND_WEM: u32 = 101;
const DELAY_ID: u32 = 14;
const INACTIVE_DELAY_ID: u32 = 15;
const BLEND_ID: u32 = 16;
const EMPTY_BLEND_ID: u32 = 17;
const TRANSITIONS: &str = "omit-container-transitions";
const BLEND_CURVES: &str = "omit-blend-curves";
const PLAIN_NODE_LEN: usize = 31;

#[test]
fn txtp_reports_repeats_endless_loops_and_loop_ranges() -> TestResult {
    let fixture = fixture()?;
    for event in [REPEAT, INFINITE, RANGE] {
        expect_simple(&fixture, event, &[LOOPS])?;
    }
    Ok(())
}

#[test]
fn txtp_reports_switch_conditions_only_on_the_path_that_uses_them() -> TestResult {
    let fixture = fixture()?;
    expect_simple(&fixture, SWITCH, &[CONDITIONS])?;
    expect_simple(&fixture, DIRECT, &[])
}

#[test]
fn txtp_reports_a_delay_only_with_an_active_transition_mode() -> TestResult {
    let fixture = fixture()?;
    expect_group(
        &fixture,
        DELAY,
        &[TRANSITIONS],
        &format!("sequence {DELAY_ID}"),
        "-S2",
    )?;
    expect_group(
        &fixture,
        INACTIVE_DELAY,
        &[],
        &format!("sequence {INACTIVE_DELAY_ID}"),
        "-S2",
    )
}

#[test]
fn txtp_reports_authored_blend_points_and_keeps_the_layer_commands() -> TestResult {
    let fixture = fixture()?;
    expect_group(
        &fixture,
        BLEND,
        &[BLEND_CURVES],
        &format!("blend {BLEND_ID}"),
        "-L2 #@layer-v",
    )?;
    expect_group(
        &fixture,
        EMPTY_BLEND,
        &[],
        &format!("blend {EMPTY_BLEND_ID}"),
        "-L2 #@layer-v",
    )
}

fn expect_group(
    fixture: &Fixture,
    event: &str,
    codes: &[&str],
    group_owner: &str,
    group_command: &str,
) -> TestResult {
    let body = format!(
        "  {event}_{WEM}.wav\n  {event}_{SECOND_WEM}.wav\n# {group_owner}\ngroup = {group_command}\n"
    );
    expect_txtp(fixture, event, codes, &body)
}

fn expect_simple(fixture: &Fixture, event: &str, codes: &[&str]) -> TestResult {
    expect_txtp(fixture, event, codes, &format!("{event}.wav\n"))
}

fn expect_txtp(fixture: &Fixture, event: &str, codes: &[&str], body: &str) -> TestResult {
    let report = extract(fixture, event, &FLAGS, 0)?;
    assert_eq!(
        report.pointer("/txtp/files/0/approximations"),
        Some(&json!(codes)),
        "only omissions on the rendered path appear for {event}",
    );
    let mut expected = format!("# P4KX: event {event} of trigger {event}\n");
    for code in codes {
        expected.push_str("# approximation: ");
        expected.push_str(code);
        expected.push('\n');
    }
    assert_eq!(
        fs::read_to_string(fixture.out.join(format!("{event}.txtp")))?,
        expected + body,
        "metadata agrees with JSON and the playback commands are retained for {event}",
    );
    if event == SWITCH {
        check_plain_fields(
            &report,
            CONDITIONS,
            "bnkHircSwitchContainer",
            &["switchDefaultValueId"],
        )?;
    }
    Ok(())
}

fn fixture() -> Result<Fixture, Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let archive = dir.path().join("Data.p4k");
    let out = dir.path().join("out");
    let mut definitions = TestAtl::new("Playback");
    let mut bank = playback_bank()?;
    for (action, (event, target)) in (1..).zip([
        (REPEAT, 10),
        (INFINITE, 11),
        (RANGE, 12),
        (SWITCH, 13),
        (DIRECT, SOUND),
        (DELAY, DELAY_ID),
        (INACTIVE_DELAY, INACTIVE_DELAY_ID),
        (BLEND, BLEND_ID),
        (EMPTY_BLEND, EMPTY_BLEND_ID),
    ]) {
        definitions = definitions.trigger(event, &[event]);
        bank = bank
            .object(TestHircObject::event(wwise_short_id(event), &[action]))
            .object(TestHircObject::action(action, PLAY, target));
    }
    ArchiveBuilder::new()
        .entry(TestEntry::zstd(
            "Data/Libs/GameAudio/Playback.xml",
            definitions.to_cryxml(),
        ))
        .entry(TestEntry::stored(
            "Data/Sounds/wwise/Playback.bnk",
            bank.bytes(),
        ))
        .write_to(&archive)?;
    Ok(Fixture {
        _dir: dir,
        archive,
        out,
    })
}

fn playback_bank() -> Result<TestBank, Box<dyn Error>> {
    Ok(TestBank::new(wwise_short_id("Playback"))
        .version(154)
        .wem(WEM, pcm())
        .wem(SECOND_WEM, pcm())
        .object(TestHircObject::sound(SOUND, WEM, EMBEDDED, 0))
        .object(TestHircObject::sound(SECOND_SOUND, SECOND_WEM, EMBEDDED, 0))
        .object(loop_container(TestContainerKind::Random, 10, [3, 0, 0])?)
        .object(loop_container(TestContainerKind::Random, 11, [0, 0, 0])?)
        .object(loop_container(TestContainerKind::Sequence, 12, [1, 0, 2])?)
        .object(TestHircObject::container(
            TestContainerKind::Switch,
            13,
            &[SOUND],
            TestNodeBase::Plain,
        ))
        .object(sequence_with_transition(DELAY_ID, 3)?)
        .object(sequence_with_transition(INACTIVE_DELAY_ID, 0)?)
        .object(TestHircObject::container(
            TestContainerKind::Blend,
            BLEND_ID,
            &CHILDREN,
            TestNodeBase::Plain,
        ))
        .object(blend_without_points(EMPTY_BLEND_ID)?))
}

fn sequence_with_transition(id: u32, mode: u8) -> Result<TestHircObject, Box<dyn Error>> {
    const TIME_OFFSET: usize = PLAIN_NODE_LEN + 3 * size_of::<u16>();
    const MODE_OFFSET: usize = TIME_OFFSET + 3 * size_of::<f32>() + size_of::<u16>();

    let mut payload = hirc_payload(TestHircObject::container(
        TestContainerKind::Sequence,
        id,
        &CHILDREN,
        TestNodeBase::Plain,
    ))?;
    assert_eq!(
        payload.get(TIME_OFFSET..TIME_OFFSET + size_of::<f32>()),
        Some(1000.0_f32.to_le_bytes().as_slice()),
        "both transition modes use the same original nonzero time"
    );
    let original_mode = payload
        .get_mut(MODE_OFFSET)
        .ok_or("original container transition mode")?;
    assert_eq!(*original_mode, 0, "the synthetic sequence starts disabled");
    *original_mode = mode;
    Ok(TestHircObject::raw(5, id, payload))
}

fn blend_without_points(id: u32) -> Result<TestHircObject, Box<dyn Error>> {
    const POINT_ROW_LEN: usize = 5 * size_of::<u32>();
    let mut payload = hirc_payload(TestHircObject::container(
        TestContainerKind::Blend,
        id,
        &CHILDREN,
        TestNodeBase::Plain,
    ))?;
    let end = payload
        .len()
        .checked_sub(size_of::<u8>())
        .ok_or("original blend validation flag")?;
    let start = end
        .checked_sub(CHILDREN.len() * POINT_ROW_LEN)
        .ok_or("original blend association rows")?;
    let rows = payload.get(start..end).ok_or("original blend point data")?;
    let authored = CHILDREN
        .into_iter()
        .flat_map(|child| [child, 1, 0, 0, 0])
        .flat_map(u32::to_le_bytes);
    assert!(
        rows.iter().copied().eq(authored),
        "both associations start with one complete zero-valued point"
    );
    let empty = CHILDREN
        .into_iter()
        .flat_map(|child| [child, 0])
        .flat_map(u32::to_le_bytes);
    drop(payload.splice(start..end, empty));
    Ok(TestHircObject::raw(9, id, payload))
}

fn loop_container(
    kind: TestContainerKind,
    id: u32,
    loops: [u16; 3],
) -> Result<TestHircObject, Box<dyn Error>> {
    let mut payload = hirc_payload(TestHircObject::container(
        kind,
        id,
        &[SOUND],
        TestNodeBase::Plain,
    ))?;
    let neutral = [1_u16, 0, 0].map(u16::to_le_bytes);
    let fields = payload
        .get_mut(PLAIN_NODE_LEN..PLAIN_NODE_LEN + size_of_val(&neutral))
        .ok_or("original loop fields after the plain node base")?;
    assert_eq!(
        fields,
        neutral.as_flattened(),
        "the range contains the original neutral loop words"
    );
    fields.copy_from_slice(loops.map(u16::to_le_bytes).as_flattened());
    Ok(TestHircObject::raw(5, id, payload))
}
