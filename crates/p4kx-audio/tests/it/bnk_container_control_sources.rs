// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_audio::{Container, ContainerLoopEvidence, ContainerSwitchEvidence, HircValue, Soundbank};
use p4kx_testkit::{TestBank, TestContainerKind, TestHircObject, TestNodeBase};

use super::{TestResult, body_of, decoded};

const PLAIN_NODE_BYTES: usize = 31;
const LOOP_WORD: usize = size_of::<u16>();
const SWITCH_WORD: usize = size_of::<u32>();
const DEFAULT_VALUE: usize = PLAIN_NODE_BYTES + size_of::<u8>() + SWITCH_WORD;
const GROUP_COUNT: usize = DEFAULT_VALUE + 3 * SWITCH_WORD + size_of::<u8>();

#[test]
fn random_sequence_records_the_first_non_neutral_loop_word() -> TestResult {
    let original = body_of(TestHircObject::container(
        TestContainerKind::Sequence,
        5,
        &[1],
        TestNodeBase::Plain,
    ))?;
    for (loops, word, expected) in [([0_u16, 2, 3], 0, 0), ([1, 2, 3], 1, 2), ([1, 0, 3], 2, 3)] {
        let mut body = original.clone();
        let fields = body
            .get_mut(PLAIN_NODE_BYTES..PLAIN_NODE_BYTES + 3 * LOOP_WORD)
            .ok_or("checked original loop words")?;
        fields.copy_from_slice(loops.map(u16::to_le_bytes).as_flattened());
        let observed = observed_container(TestHircObject::raw(5, 5, body))?;
        let (observed_word, field) = match observed.container.loop_evidence() {
            Some(ContainerLoopEvidence::Count(field)) => (0, field),
            Some(ContainerLoopEvidence::Minimum(field)) => (1, field),
            Some(ContainerLoopEvidence::Maximum(field)) => (2, field),
            _ => return Err("expected original non-neutral loop evidence".into()),
        };
        assert_eq!(observed_word, word, "first non-neutral original field");
        assert_eq!(field.value, expected, "original loop word value");
        let offset = PLAIN_NODE_BYTES + word * LOOP_WORD;
        observed.check_word(&field.range, offset, &expected.to_le_bytes());
    }
    Ok(())
}

#[test]
fn a_switch_without_a_default_cites_the_checked_nonempty_membership_count() -> TestResult {
    let original = body_of(TestHircObject::container(
        TestContainerKind::Switch,
        5,
        &[1],
        TestNodeBase::Plain,
    ))?;
    let observed = observed_container(TestHircObject::raw(6, 5, original.clone()))?;
    let Some(ContainerSwitchEvidence::DefaultValue(default)) =
        observed.container.switch_condition_evidence()
    else {
        return Err("a nonzero default precedes the group count".into());
    };
    assert_eq!(default.value, 0x3779_8A64, "original switch default");
    observed.check_word(&default.range, DEFAULT_VALUE, &default.value.to_le_bytes());

    let mut body = original;
    body.get_mut(DEFAULT_VALUE..DEFAULT_VALUE + SWITCH_WORD)
        .ok_or("original default value word")?
        .fill(0);
    let observed = observed_container(TestHircObject::raw(6, 5, body.clone()))?;
    let Some(ContainerSwitchEvidence::MembershipGroupCount(count)) =
        observed.container.switch_condition_evidence()
    else {
        return Err("a checked switch group supplies its declared count".into());
    };
    assert_eq!(count.value, 1, "one complete group is present");
    observed.check_word(&count.range, GROUP_COUNT, &1_u32.to_le_bytes());

    body.get_mut(GROUP_COUNT..GROUP_COUNT + SWITCH_WORD)
        .ok_or("original group count word")?
        .copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(
        matches!(
            decoded(TestHircObject::raw(6, 5, body), 154)?.1,
            HircValue::Malformed(_)
        ),
        "an incomplete group table cannot supply a positioned condition",
    );
    Ok(())
}

struct ObservedContainer {
    bytes: Vec<u8>,
    body_start: usize,
    container: Container,
}

impl ObservedContainer {
    fn check_word(&self, original: &Range<u64>, body_offset: usize, bytes: &[u8]) {
        let start = self.body_start + body_offset;
        let end = start + bytes.len();
        assert_eq!(
            *original,
            start as u64..end as u64,
            "bank-relative scalar span"
        );
        assert_eq!(
            self.bytes.get(start..end),
            Some(bytes),
            "original scalar bytes"
        );
    }
}

fn observed_container(
    container: TestHircObject,
) -> Result<ObservedContainer, Box<dyn std::error::Error>> {
    let bytes = TestBank::new(1)
        .object(TestHircObject::raw(0xa0, 91, [0; 13]))
        .object(container)
        .bytes();
    let bank = Soundbank::parse(bytes.as_slice())?;
    let object = bank.objects().last().ok_or("second physical object")?;
    let HircValue::Container(container) = &object.value else {
        return Err("expected a checked container".into());
    };
    let body_start = object.id_byte_range().end;
    let container = container.clone();
    Ok(ObservedContainer {
        bytes,
        body_start,
        container,
    })
}
