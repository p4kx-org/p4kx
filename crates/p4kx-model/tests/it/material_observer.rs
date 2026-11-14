// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::Cell;
use std::convert::Infallible;

use p4kx_model::{
    MAX_SUB_MATERIALS, MaterialError, MaterialLibrary, MaterialReadError, MaterialReadField,
    MaterialReadUse, MaterialSourceLimits,
};

use crate::material_observer_baseline::{self as baseline, TestResult};
use crate::material_observer_support as support;

#[test]
fn no_op_material_observer_preserves_payloads_and_checkpoint_sequence() -> TestResult {
    for bytes in baseline::samples()
        .into_iter()
        .chain(baseline::stencil_samples())
    {
        let plain_calls = Cell::new(0);
        let plain = MaterialLibrary::parse_with(
            "synthetic/observer",
            &bytes,
            MaterialSourceLimits::default(),
            || {
                plain_calls.set(plain_calls.get() + 1);
                Ok::<(), Infallible>(())
            },
        )?;
        let observed_calls = Cell::new(0);
        let observed = MaterialLibrary::parse_with_observer(
            "synthetic/observer",
            &bytes,
            MaterialSourceLimits::default(),
            || {
                observed_calls.set(observed_calls.get() + 1);
                Ok::<(), Infallible>(())
            },
            |_| Ok(()),
        )?;
        assert_eq!(
            observed, plain,
            "the observer does not change retained syntax or interpreted payloads"
        );
        assert_eq!(
            observed_calls.get(),
            plain_calls.get(),
            "all existing checkpoints keep their order and multiplicity"
        );
    }
    Ok(())
}

#[test]
fn material_observer_distinguishes_explicit_default_values_from_missing_or_invalid_inputs()
-> TestResult {
    for (attributes, expected) in [
        ("", MaterialReadUse::Selected),
        ("Opacity='bad'", MaterialReadUse::Selected),
        ("Opacity='1'", MaterialReadUse::Interpreted),
    ] {
        let xml = format!("<Material Name='' {attributes}/>");
        let (library, events) = support::capture(xml.as_bytes(), MaterialSourceLimits::default())?;
        assert_eq!(
            library.materials.first().ok_or("material")?.opacity,
            1.0,
            "equal output values do not identify the original branch"
        );
        let opacity = support::field(&events, MaterialReadField::Opacity, "Opacity")?;
        assert_eq!(
            opacity.use_kind, expected,
            "the actual parse/default branch is observed"
        );
        assert!(
            opacity.indices.is_empty(),
            "a single material uses the original root site"
        );
        assert_eq!(
            support::field(&events, MaterialReadField::Name, "Name")?.use_kind,
            MaterialReadUse::Authored,
            "explicit empty text remains authored"
        );
        assert_eq!(
            support::field(&events, MaterialReadField::Flags, "MtlFlags")?.use_kind,
            MaterialReadUse::Selected,
            "missing flags retain their default selector"
        );
    }
    Ok(())
}

#[test]
fn material_observer_retains_unknown_slot_disposition_for_every_interpreted_output() -> TestResult {
    let (_, events) = support::capture(
        b"<Material><Ignored/><SubMaterials><MatRef/></SubMaterials></Material>",
        MaterialSourceLimits::default(),
    )?;
    let unknown: Vec<_> = events
        .iter()
        .filter(|event| event.source_field == "MatRef")
        .collect();
    assert_eq!(
        unknown.len(),
        13,
        "each interpreted engine-default output retains the slot selector"
    );
    support::assert_default_fields(&unknown);
    for event in unknown {
        assert_eq!(
            event.material, 0,
            "the first unknown slot retains its material ID"
        );
        assert_eq!(
            event.indices,
            [1, 0],
            "unrelated root children do not compact the source path"
        );
        assert_eq!(
            event.use_kind,
            MaterialReadUse::Selected,
            "engine defaults are selected by source disposition"
        );
    }
    assert!(
        !events.iter().any(|event| event.source_field == "Opacity"),
        "unknown slots do not claim a typed attribute read"
    );
    Ok(())
}

#[test]
fn material_observer_uses_live_original_sites_when_authored_retention_is_cut() -> TestResult {
    let mut limits = MaterialSourceLimits::default();
    limits.text_bytes = 1;
    let (library, events) = support::capture(
        b"<Material><SubMaterials><Material Opacity='0.5'/></SubMaterials></Material>",
        limits,
    )?;
    assert!(
        library.authored.root.is_none(),
        "the independent authored tree cannot fit"
    );
    let opacity = support::field(&events, MaterialReadField::Opacity, "Opacity")?;
    assert_eq!(
        opacity.indices,
        [0, 0],
        "the actual parser supplies source location without the retained tree"
    );
    assert_eq!(
        opacity.use_kind,
        MaterialReadUse::Interpreted,
        "the parsed value retains its actual input"
    );
    Ok(())
}

#[test]
fn material_observer_failure_keeps_original_interruption_and_stops_callbacks() {
    let calls = Cell::new(0);
    let result = MaterialLibrary::parse_with_observer(
        "synthetic/stop",
        b"<Material/>",
        MaterialSourceLimits::default(),
        || Ok(()),
        |_| {
            calls.set(calls.get() + 1);
            Err("observer")
        },
    );
    assert!(
        matches!(result, Err(MaterialReadError::Interrupted("observer"))),
        "observer failure is not a material defect"
    );
    assert_eq!(calls.get(), 1, "no callback follows the first interruption");
    let calls = Cell::new(0);
    let result = MaterialLibrary::parse_with_observer(
        "synthetic/stop",
        b"<Material/>",
        MaterialSourceLimits::default(),
        || Err("checkpoint"),
        |_| {
            calls.set(calls.get() + 1);
            Ok(())
        },
    );
    assert!(
        matches!(result, Err(MaterialReadError::Interrupted("checkpoint"))),
        "the existing checkpoint still wins"
    );
    assert_eq!(
        calls.get(),
        0,
        "no material fields are observed before source admission"
    );
}

#[test]
fn optional_material_observation_capture_can_stop_retaining_without_aborting_payload() -> TestResult
{
    let bytes = baseline::samples().into_iter().next().ok_or("sample")?;
    let expected = MaterialLibrary::parse_with(
        "synthetic/observer",
        &bytes,
        MaterialSourceLimits::default(),
        || Ok::<(), Infallible>(()),
    )?;
    let mut retained = None;
    let mut seen = 0;
    let actual = MaterialLibrary::parse_with_observer(
        "synthetic/observer",
        &bytes,
        MaterialSourceLimits::default(),
        || Ok::<(), Infallible>(()),
        |event| {
            seen += 1;
            if retained.is_none() {
                retained = Some((event.material, event.indices.to_vec()));
            }
            Ok(())
        },
    )?;
    assert_eq!(
        actual, expected,
        "optional capture admission remains a caller concern"
    );
    assert!(
        retained.is_some() && seen > 1,
        "a retained witness does not prevent later payload work"
    );
    Ok(())
}

#[test]
fn material_observer_does_not_publish_field_results_for_rejected_configuration_or_source() {
    let calls = Cell::new(0);
    let mut limits = MaterialSourceLimits::default();
    limits.elements = Some(0);
    let result = MaterialLibrary::parse_with_observer(
        "synthetic/invalid",
        b"invalid",
        limits,
        || Err("checkpoint"),
        |_| {
            calls.set(calls.get() + 1);
            Err("observer")
        },
    );
    assert!(
        matches!(
            result,
            Err(MaterialReadError::Material(
                MaterialError::InvalidSourceLimits
            ))
        ),
        "configuration rejection precedes observations and interruptions"
    );
    assert_eq!(
        calls.get(),
        0,
        "rejected configuration has no interpreted field observations"
    );
    let result = MaterialLibrary::parse_with_observer(
        "synthetic/root",
        b"<Other/>",
        MaterialSourceLimits::default(),
        || Ok::<(), Infallible>(()),
        |_| {
            calls.set(calls.get() + 1);
            Ok(())
        },
    );
    assert!(
        matches!(
            result,
            Err(MaterialReadError::Material(
                MaterialError::NotMaterial { .. }
            ))
        ),
        "root rejection remains a source-format failure"
    );
    assert_eq!(
        calls.get(),
        0,
        "a non-material root has no interpreted fields"
    );
}

#[test]
fn material_observer_keeps_all_texture_slots_with_original_observations() -> TestResult {
    crate::material_observer_texture_sites::check()
}

#[test]
fn material_observer_keeps_a_typed_failure_after_actual_partial_observations() {
    let xml = format!(
        "<Material><SubMaterials>{}</SubMaterials></Material>",
        "<Material/>".repeat(MAX_SUB_MATERIALS + 1)
    );
    let calls = Cell::new(0);
    let result = MaterialLibrary::parse_with_observer(
        "synthetic/submaterial-limit",
        xml.as_bytes(),
        MaterialSourceLimits::default(),
        || Ok::<(), Infallible>(()),
        |_| {
            calls.set(calls.get() + 1);
            Ok(())
        },
    );
    assert!(
        matches!(
            result,
            Err(MaterialReadError::Material(MaterialError::TooMany {
                what: "sub-materials",
                limit: MAX_SUB_MATERIALS,
            }))
        ),
        "a genuine typed input failure remains a material error"
    );
    assert!(
        calls.get() > 0,
        "earlier original observations precede the failure"
    );
}

#[test]
fn material_observer_distinguishes_equal_vector_and_flag_defaults_by_actual_parse_branch()
-> TestResult {
    for (attributes, expected) in [
        ("", MaterialReadUse::Selected),
        ("Diffuse='bad' MtlFlags='bad'", MaterialReadUse::Selected),
        ("Diffuse='1,1,1' MtlFlags='0'", MaterialReadUse::Interpreted),
    ] {
        let xml = format!("<Material {attributes}/>");
        let (library, events) = support::capture(xml.as_bytes(), MaterialSourceLimits::default())?;
        let material = library.materials.first().ok_or("material")?;
        assert_eq!(
            material.diffuse, [1.0; 3],
            "equal output colours can have distinct source branches"
        );
        assert_eq!(
            material.flags.0, 0,
            "equal output flags can have distinct source branches"
        );
        assert_eq!(
            support::field(&events, MaterialReadField::Diffuse, "Diffuse")?.use_kind,
            expected,
            "vector conversion and fallback retain distinct actual operations"
        );
        assert_eq!(
            support::field(&events, MaterialReadField::Flags, "MtlFlags")?.use_kind,
            expected,
            "flags conversion and fallback retain distinct actual operations"
        );
    }
    Ok(())
}
