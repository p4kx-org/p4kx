// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::convert::Infallible;

use p4kx_model::{
    MaterialError, MaterialLibrary, MaterialReadError, MaterialSourceLimits, MaterialSourceLossKind,
};

use super::{
    CHILDREN, TestResult, assert_fragment, assert_material_fragments, assert_original, original_xml,
};

#[test]
fn authored_element_default_none_is_shared_by_existing_checkpoint_and_observer_routes() -> TestResult
{
    let xml = original_xml()?;
    assert_original(&xml)?;
    let limits = MaterialSourceLimits::default();
    assert_eq!(
        limits.elements, None,
        "default source policy has no arbitrary element prefix"
    );
    let library = MaterialLibrary::parse_with("synthetic/policy", xml.as_bytes(), limits, || {
        Ok::<(), Infallible>(())
    })?;
    assert_complete(&library)?;
    let mut observed = 0;
    let library = MaterialLibrary::parse_with_observer(
        "synthetic/policy",
        xml.as_bytes(),
        limits,
        || Ok::<(), Infallible>(()),
        |_| {
            observed += 1;
            Ok(())
        },
    )?;
    assert!(observed > 0, "the real existing observer route runs");
    assert_complete(&library)
}

#[test]
fn authored_element_explicit_some4096_keeps_the_original_finite_boundary() -> TestResult {
    let xml = original_xml()?;
    assert_original(&xml)?;
    let mut limits = MaterialSourceLimits::default();
    limits.elements = Some(4096);
    let library =
        MaterialLibrary::parse_with("synthetic/explicit", xml.as_bytes(), limits, || {
            Ok::<(), Infallible>(())
        })?;
    assert_material_fragments(&library)?;
    let root = library
        .authored
        .root
        .as_ref()
        .ok_or("actual finite-bound root")?;
    assert_eq!(
        root.children.len(),
        4095,
        "the explicit4096 budget still includes the root"
    );
    for (child, ordinal) in root.children.iter().zip(0..4095) {
        assert_fragment(child, ordinal);
        assert_eq!(
            child.authored.ordinal,
            Some(ordinal),
            "finite clipping does not renumber retained source"
        );
    }
    let kinds: Vec<_> = library
        .authored
        .losses
        .iter()
        .map(|loss| loss.kind)
        .collect();
    assert_eq!(
        kinds,
        [MaterialSourceLossKind::Elements],
        "explicit finite prefix remains honestly incomplete"
    );
    Ok(())
}

#[test]
fn authored_element_explicit_some0_keeps_invalid_limits_before_checkpoint() -> TestResult {
    let xml = original_xml()?;
    assert_original(&xml)?;
    let mut limits = MaterialSourceLimits::default();
    limits.elements = Some(0);
    let mut calls = 0;
    let result = MaterialLibrary::parse_with("synthetic/zero", xml.as_bytes(), limits, || {
        calls += 1;
        Ok::<(), Infallible>(())
    });
    assert!(
        matches!(
            result,
            Err(MaterialReadError::Material(
                MaterialError::InvalidSourceLimits
            ))
        ),
        "explicit zero retains the original typed invalid-limit contract"
    );
    assert_eq!(
        calls, 0,
        "limit validation still precedes the original checkpoint"
    );
    Ok(())
}

fn assert_complete(library: &MaterialLibrary) -> TestResult {
    assert_material_fragments(library)?;
    let root = library
        .authored
        .root
        .as_ref()
        .ok_or("actual default root")?;
    assert_eq!(
        root.children.len(),
        CHILDREN,
        "every original child is retained through the same default policy"
    );
    for (child, ordinal) in root.children.iter().zip(0..CHILDREN) {
        assert_fragment(child, ordinal);
        assert_eq!(
            child.authored.ordinal,
            Some(ordinal),
            "full original ordinal sequence"
        );
    }
    assert!(
        library.authored.losses.is_empty(),
        "default complete-element route has no source loss for this fixture"
    );
    Ok(())
}
