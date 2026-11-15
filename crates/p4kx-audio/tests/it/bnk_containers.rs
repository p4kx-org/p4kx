// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Random, sequence, switch and blend containers of the `HIRC` section, decoded to their children.

use p4kx_audio::{BankError, ContainerKind, HircKind, HircValue, SectionId, Soundbank};
use p4kx_testkit::{TestBank, TestContainerKind, TestHircObject, TestNodeBase};

type TestResult = Result<(), Box<dyn std::error::Error>>;

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
    let bank = Soundbank::parse(&bytes)?;
    let object = bank.objects().first().ok_or("original container")?;
    Ok(bank
        .object_body(object)
        .get(size_of::<u32>()..)
        .ok_or("object body after ID")?
        .to_vec())
}

fn parts(value: &HircValue) -> Option<(ContainerKind, u32, Vec<u32>)> {
    match value {
        HircValue::Container(container) => Some((
            container.kind,
            container.parent_id,
            container.children.clone(),
        )),
        _ => None,
    }
}

#[test]
fn every_container_kind_is_decoded_with_its_children_in_order() -> TestResult {
    for (kind, base, expected_kind, type_byte) in [
        (
            TestContainerKind::Random,
            TestNodeBase::Plain,
            ContainerKind::Random,
            HircKind::RandomSequenceContainer,
        ),
        (
            TestContainerKind::Sequence,
            TestNodeBase::Full,
            ContainerKind::Sequence,
            HircKind::RandomSequenceContainer,
        ),
        (
            TestContainerKind::Switch,
            TestNodeBase::Full,
            ContainerKind::Switch,
            HircKind::SwitchContainer,
        ),
        (
            TestContainerKind::Blend,
            TestNodeBase::Plain,
            ContainerKind::Blend,
            HircKind::LayerContainer,
        ),
    ] {
        let object = TestHircObject::container(kind, 77, &[30, 10, 20], base);
        let (decoded_kind, value) = decoded(object, 154)?;
        assert_eq!(decoded_kind, type_byte, "{kind:?} object kind");
        assert_eq!(
            parts(&value),
            Some((expected_kind, 0x2C21_1B2E, vec![30, 10, 20])),
            "{kind:?} with a {base:?} node base"
        );
        let HircValue::Container(container) = value else {
            return Err("eager container".into());
        };
        assert!(
            !container.has_authored_loops,
            "neutral loop control for {kind:?}"
        );
        assert_eq!(
            container.has_switch_conditions,
            kind == TestContainerKind::Switch,
            "only Switch fixtures author membership and a default value",
        );
        assert!(
            !container.has_authored_transitions(),
            "a disabled mode leaves nonzero timing words inactive"
        );
        assert_eq!(
            container.has_blend_curves(),
            kind == TestContainerKind::Blend,
            "all-zero point values still form an authored Blend curve"
        );
    }
    Ok(())
}

#[test]
fn each_nonzero_random_sequence_transition_mode_is_observed() -> TestResult {
    const PLAIN_NODE_BYTES: usize = 31;
    const TRANSITION_MODE: usize = PLAIN_NODE_BYTES + 4 * size_of::<u16>() + 3 * size_of::<u32>();
    let object =
        TestHircObject::container(TestContainerKind::Sequence, 5, &[1], TestNodeBase::Plain);
    let original = body_of(object)?;
    for mode in [3, u8::MAX] {
        let mut body = original.clone();
        let field = body
            .get_mut(TRANSITION_MODE)
            .ok_or("checked transition mode byte")?;
        assert_eq!(*field, 0, "synthetic base mode is disabled");
        *field = mode;
        let bytes = TestBank::new(1)
            .object(TestHircObject::raw(0xa0, 91, [0; 13]))
            .object(TestHircObject::raw(5, 5, body))
            .bytes();
        let bank = Soundbank::parse(&bytes)?;
        let object = bank.objects().last().ok_or("second object")?;
        let HircValue::Container(container) = &object.value else {
            return Err("expected a checked Random/Sequence transition mode".into());
        };
        assert!(container.has_authored_transitions(), "nonzero mode {mode}");
        let source = container
            .transition_mode_evidence()
            .ok_or("mode evidence")?;
        let start = object.id_byte_range().end + TRANSITION_MODE;
        assert_eq!(source.value, mode, "original mode is kept");
        assert_eq!(
            source.range,
            start as u64..(start + 1) as u64,
            "only the original mode byte is cited"
        );
        assert_eq!(bytes.get(start), Some(&mode), "position is bank-relative");
    }
    Ok(())
}

#[test]
fn a_blend_controller_with_an_empty_association_curve_is_neutral() -> TestResult {
    let object = TestHircObject::container(TestContainerKind::Blend, 5, &[1], TestNodeBase::Plain);
    let mut body = body_of(object)?;
    let original = [1_u32, 0, 0, 0].map(u32::to_le_bytes);
    let end = body
        .len()
        .checked_sub(1)
        .ok_or("continuous-validation byte")?;
    let start = end
        .checked_sub(size_of_val(&original))
        .ok_or("original association point count and point")?;
    assert_eq!(
        body.get(start..end),
        Some(original.as_flattened()),
        "the one association has a checked all-zero point"
    );
    drop(body.splice(start..end, 0_u32.to_le_bytes()));
    let (_, value) = decoded(TestHircObject::raw(9, 5, body), 154)?;
    let HircValue::Container(container) = value else {
        return Err("expected a checked Layer with a controller and empty curve".into());
    };
    assert!(!container.has_blend_curves(), "no original curve points");
    Ok(())
}

#[test]
fn a_blend_reports_the_first_positive_checked_point_count_in_bank_order() -> TestResult {
    const CHILDREN: [u32; 3] = [101, 102, 103];
    const WORD: usize = size_of::<u32>();
    let mut body = body_of(TestHircObject::container(
        TestContainerKind::Blend,
        5,
        &CHILDREN,
        TestNodeBase::Plain,
    ))?;
    let end = body
        .len()
        .checked_sub(1)
        .ok_or("continuous-validation byte")?;
    let start = end
        .checked_sub(CHILDREN.len() * 5 * WORD)
        .ok_or("association rows")?;
    let expected = CHILDREN
        .into_iter()
        .flat_map(|child| [child, 1, 0, 0, 0])
        .flat_map(u32::to_le_bytes);
    assert!(
        body.get(start..end)
            .ok_or("original association range")?
            .iter()
            .copied()
            .eq(expected),
        "the fixture starts with one complete point per association"
    );
    let replacement = [101_u32, 0, 102, 2, 0, 0, 0, 0, 0, 0, 103, 1, 0, 0, 0];
    drop(body.splice(
        start..end,
        replacement.into_iter().flat_map(u32::to_le_bytes),
    ));
    let bytes = TestBank::new(1)
        .object(TestHircObject::raw(9, 5, body))
        .bytes();
    let bank = Soundbank::parse(&bytes)?;
    let object = bank.objects().first().ok_or("layer container")?;
    let HircValue::Container(container) = &object.value else {
        return Err("expected a checked Layer with populated associations".into());
    };
    let source = container
        .blend_point_count_evidence()
        .ok_or("point count evidence")?;
    let field_start = object.id_byte_range().end + start + 3 * WORD;
    let field_end = field_start + WORD;
    assert_eq!(source.value, 2, "the empty first association is skipped");
    assert_eq!(
        source.range,
        field_start as u64..field_end as u64,
        "the second association supplies the first positive count"
    );
    assert_eq!(
        bytes.get(field_start..field_end),
        Some(2_u32.to_le_bytes().as_slice()),
        "only the exact original count word is cited"
    );
    Ok(())
}

#[test]
fn a_container_without_children_is_decoded_empty() -> TestResult {
    let object = TestHircObject::container(TestContainerKind::Switch, 5, &[], TestNodeBase::Plain);
    assert_eq!(
        parts(&decoded(object, 154)?.1),
        Some((ContainerKind::Switch, 0x2C21_1B2E, Vec::new())),
        "no children"
    );
    Ok(())
}

#[test]
fn a_container_of_another_bank_version_stays_raw() -> TestResult {
    let object = TestHircObject::container(TestContainerKind::Random, 5, &[1], TestNodeBase::Plain);
    assert_eq!(
        decoded(object, 150)?,
        (HircKind::RandomSequenceContainer, HircValue::Raw),
        "layout unknown"
    );
    Ok(())
}

#[test]
fn a_truncated_or_overlong_container_is_malformed_and_the_bank_still_parses() -> TestResult {
    let full = TestHircObject::container(TestContainerKind::Blend, 5, &[1, 2], TestNodeBase::Full);
    let body = body_of(full)?;
    for (name, cut) in [
        (
            "truncated",
            body.get(..body.len() - 3).unwrap_or_default().to_vec(),
        ),
        ("overlong", [body.as_slice(), &[0]].concat()),
        (
            "half",
            body.get(..body.len() / 2).unwrap_or_default().to_vec(),
        ),
    ] {
        let (kind, value) = decoded(TestHircObject::raw(9, 5, cut), 154)?;
        assert_eq!(kind, HircKind::LayerContainer, "{name}: kind kept");
        assert!(
            matches!(
                value,
                HircValue::Malformed(BankError::Truncated { section, .. }
                    | BankError::InvalidSection { section, .. }) if section == SectionId::HIRC
            ),
            "{name}: {value:?}"
        );
    }
    Ok(())
}

#[test]
fn a_child_count_beyond_the_body_is_malformed_without_allocating_it() -> TestResult {
    let mut body = vec![0; 8];
    body.extend_from_slice(&0x2C21_1B2E_u32.to_le_bytes());
    body.extend_from_slice(&[0; 5]);
    body.extend_from_slice(&[0; 4]);
    body.extend_from_slice(&[0, 1, 0, 0, 3, 0x0C]);
    body.extend_from_slice(&[0, 0, 0, 0]);
    body.extend_from_slice(&u32::MAX.to_le_bytes());
    let (_, value) = decoded(TestHircObject::raw(9, 5, body), 154)?;
    assert!(
        matches!(
            value,
            HircValue::Malformed(BankError::InvalidSection { .. })
        ),
        "{value:?}"
    );
    Ok(())
}

#[test]
fn a_plugin_source_is_told_apart_from_a_codec_source() -> TestResult {
    let bytes = TestBank::new(1)
        .object(TestHircObject::plugin_sound(1, 2))
        .object(TestHircObject::sound(3, 4, 0, 5))
        .bytes();
    let bank = Soundbank::parse(bytes.as_slice())?;
    let plugins: Vec<bool> = bank
        .objects()
        .iter()
        .filter_map(|object| match &object.value {
            HircValue::Sound(source) => Some(source.is_source_plugin()),
            _ => None,
        })
        .collect();
    assert_eq!(plugins, [true, false], "plug-in, then Vorbis");
    Ok(())
}
