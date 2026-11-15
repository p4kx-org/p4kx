// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;

use p4kx_testkit::{
    ArchiveBuilder, TestAtl, TestBank, TestContainerKind, TestEntry, TestHircObject, TestNodeBase,
    wwise_short_id,
};
use serde_json::json;

use super::{EMBEDDED, Fixture, PLAY, TestResult, extract, hirc_payload, pcm};

const REPEAT: &str = "Play_Repeat_Container";
const INFINITE: &str = "Play_Infinite_Container";
const RANGE: &str = "Play_Loop_Range";
const SWITCH: &str = "Play_Conditioned_Container";
const DIRECT: &str = "Play_Same_Sound_Directly";
const FLAGS: [&str; 3] = ["--format", "wav", "--txtp"];
const LOOPS: &str = "omit-container-loops";
const CONDITIONS: &str = "omit-container-conditions";
const SOUND: u32 = 20;
const WEM: u32 = 100;
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

fn expect_simple(fixture: &Fixture, event: &str, codes: &[&str]) -> TestResult {
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
        expected + &format!("{event}.wav\n"),
        "metadata agrees with JSON and the one-child body is retained",
    );
    Ok(())
}

fn fixture() -> Result<Fixture, Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let archive = dir.path().join("Data.p4k");
    let out = dir.path().join("out");
    let mut definitions = TestAtl::new("Playback");
    let mut bank = TestBank::new(wwise_short_id("Playback"))
        .version(154)
        .wem(WEM, pcm())
        .object(TestHircObject::sound(SOUND, WEM, EMBEDDED, 0))
        .object(loop_container(TestContainerKind::Random, 10, [3, 0, 0])?)
        .object(loop_container(TestContainerKind::Random, 11, [0, 0, 0])?)
        .object(loop_container(TestContainerKind::Sequence, 12, [1, 0, 2])?)
        .object(TestHircObject::container(
            TestContainerKind::Switch,
            13,
            &[SOUND],
            TestNodeBase::Plain,
        ));
    for (action, (event, target)) in (1..).zip([
        (REPEAT, 10),
        (INFINITE, 11),
        (RANGE, 12),
        (SWITCH, 13),
        (DIRECT, SOUND),
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
