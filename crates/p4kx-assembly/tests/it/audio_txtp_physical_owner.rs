// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{TestBank, TestContainerKind, TestHircObject, wwise_short_id};

use super::audio_txtp_support::{Case, EMBEDDED, PLAY, TestResult, container, pcm, read, sound};

const FIRST_WEMS: [u32; 2] = [7_001, 7_002];
const SECOND_WEMS: [u32; 2] = [7_003, 7_004];
const CONTAINER: u32 = 10;

#[test]
fn identical_short_ids_in_distinct_event_holders_keep_independent_txtp_groups() -> TestResult {
    let mut second = TestBank::new(wwise_short_id("Second")).version(154);
    for object in event_objects(SECOND_WEMS, &[1]) {
        second = second.object(object);
    }
    for id in SECOND_WEMS {
        second = second.wem(id, pcm().bytes());
    }
    Case {
        wems: FIRST_WEMS
            .into_iter()
            .map(|id| (id, pcm().bytes()))
            .collect(),
        objects: event_objects(FIRST_WEMS, &[1]),
        additional_banks: vec![("Second.bnk".to_owned(), second.bytes())],
        ..Case::default()
    }
    .run(|folder, report, _| {
        assert_eq!(
            report.files.len(),
            1,
            "same logical event produces one plan"
        );
        independent_choices(&read(folder, "Play_X.txtp")?);
        Ok(())
    })
}

#[test]
fn repeated_authored_action_edges_keep_independent_txtp_groups() -> TestResult {
    Case {
        wems: FIRST_WEMS
            .into_iter()
            .map(|id| (id, pcm().bytes()))
            .collect(),
        objects: event_objects(FIRST_WEMS, &[1, 1]),
        ..Case::default()
    }
    .run(|folder, report, _| {
        assert_eq!(report.files.len(), 1, "one event plan");
        independent_choices(&read(folder, "Play_X.txtp")?);
        Ok(())
    })
}

fn independent_choices(text: &str) {
    let groups: Vec<_> = text
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("group = "))
        .collect();
    assert_eq!(
        groups,
        ["group = -R2>1", "group = -R2>1", "group = -L2 #@layer-v"],
        "each original Random occurrence selects within its own group: {text}",
    );
}

fn event_objects([first, second]: [u32; 2], actions: &[u32]) -> Vec<TestHircObject> {
    vec![
        TestHircObject::event(wwise_short_id("Play_X"), actions),
        TestHircObject::action(1, PLAY, CONTAINER),
        container(TestContainerKind::Random, CONTAINER, &[20, 21]),
        sound(20, first, EMBEDDED, 0),
        sound(21, second, EMBEDDED, 0),
    ]
}
