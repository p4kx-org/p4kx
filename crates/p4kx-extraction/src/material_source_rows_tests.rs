// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::convert::Infallible;
use std::error::Error;

use p4kx_model::{MaterialLibrary, MaterialSourceLimits};
use p4kx_testkit::{CryXmlByteOrder, CryXmlSignature, TestElement};

use crate::{MaterialElementPart, MaterialElementRow, MaterialLibrarySourceInfo};

type TestResult = Result<(), Box<dyn Error>>;

const RICH: &[u8] = br#"<?xml version="1.0"?>
<Material Shader="Synthetic" x:Token="a&amp;b" xmlns:x="urn:synthetic">
  alpha<![CDATA[<>&]]><!--comment--><?proc test?>
  <SubMaterials><Opaque X="1" x="2"/><Material Name="Sub"><Layer/></Material></SubMaterials>
</Material>"#;

#[test]
fn source_rows_reconstitute_canonical_text_and_binary_wire() -> TestResult {
    let binary = TestElement::new("Material")
        .attribute("Shader", "Synthetic")
        .child(
            TestElement::new("SubMaterials").child(
                TestElement::new("Custom")
                    .attribute("UP", "1")
                    .attribute("up", "2"),
            ),
        );
    for bytes in [
        RICH.to_vec(),
        binary.to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little),
    ] {
        let library = MaterialLibrary::parse("synthetic/material", &bytes)?;
        let original = MaterialLibrarySourceInfo::of(&library);
        let raw = library.authored.root.as_ref().ok_or("retained root")?;
        let mut wire = Vec::new();
        MaterialElementRow::visit(raw, |_original, part| {
            wire.push(serde_json::to_vec(&part)?);
            Ok::<(), serde_json::Error>(())
        })?;
        let rows = wire
            .iter()
            .map(|value| serde_json::from_slice::<MaterialElementPart<'_>>(value))
            .collect::<Result<Vec<_>, _>>()?;
        assert_eq!(
            Some(MaterialElementRow::reconstruct(rows)?),
            original.root,
            "round trips every original retained declaration and member"
        );
    }
    Ok(())
}

#[test]
fn reconstruction_rejects_removed_duplicate_or_misplaced_rows() -> TestResult {
    let library = MaterialLibrary::parse("synthetic/material", RICH)?;
    let raw = library.authored.root.as_ref().ok_or("retained root")?;
    let mut parts = Vec::new();
    MaterialElementRow::visit(raw, |_original, part| {
        parts.push(part);
        Ok::<(), Infallible>(())
    })?;
    for omitted in 0..parts.len() {
        let rows = parts
            .iter()
            .enumerate()
            .filter(|(position, _)| *position != omitted)
            .map(|(_, part)| part.clone());
        assert!(
            MaterialElementRow::reconstruct(rows).is_err(),
            "missing value is rejected at {omitted}"
        );
    }
    let first = parts.first().ok_or("first header")?;
    let duplicated = parts.iter().chain(std::iter::once(first)).cloned();
    assert!(
        MaterialElementRow::reconstruct(duplicated).is_err(),
        "extra header is rejected"
    );
    let mut misplaced = parts.clone();
    let header = misplaced
        .iter_mut()
        .find(|part| part.element == 1)
        .ok_or("child header")?;
    let MaterialElementRow::Element {
        parent: Some(parent),
        ..
    } = &mut header.row
    else {
        return Err("first child header missing".into());
    };
    parent.element = 99;
    assert!(
        MaterialElementRow::reconstruct(misplaced).is_err(),
        "unobserved parent is rejected"
    );
    Ok(())
}

#[test]
fn retained_child_positions_do_not_erase_original_ordinal() -> TestResult {
    let mut limits = MaterialSourceLimits::default();
    limits.elements = Some(3);
    let mut library = MaterialLibrary::parse_with(
        "synthetic/material",
        b"<Material><A/><B/></Material>",
        limits,
        || Ok::<(), Infallible>(()),
    )?;
    let captured = std::sync::Arc::make_mut(&mut library.authored)
        .root
        .as_mut()
        .ok_or("retained root")?;
    captured.children.remove(0);
    let original = MaterialLibrarySourceInfo::of(&library);
    let raw = library.authored.root.as_ref().ok_or("retained root")?;
    let mut parts = Vec::new();
    let mut source_ordinals = Vec::new();
    MaterialElementRow::visit(raw, |original, part| {
        if matches!(part.row, MaterialElementRow::Element { .. }) {
            source_ordinals.push(original.map(<[usize]>::to_vec));
        }
        parts.push(part);
        Ok::<(), Infallible>(())
    })?;
    assert_eq!(
        source_ordinals,
        [Some(Vec::new()), Some(vec![1])],
        "original child ordinal survives a retained position gap"
    );
    assert_eq!(
        Some(MaterialElementRow::reconstruct(parts)?),
        original.root,
        "bounded retained structure remains exact"
    );
    Ok(())
}

#[test]
fn traversal_preserves_the_exact_interruption() -> TestResult {
    let library = MaterialLibrary::parse("synthetic/material", RICH)?;
    let root = library.authored.root.as_ref().ok_or("retained root")?;
    let mut observed = 0;
    let result = MaterialElementRow::visit(root, |_, _| {
        observed += 1;
        if observed == 3 {
            Err("caller stopped")
        } else {
            Ok(())
        }
    });
    assert!(
        matches!(
            result,
            Err(crate::MaterialElementWriteError::Interrupted(
                "caller stopped"
            ))
        ),
        "the callback retains its exact stop outcome"
    );
    assert_eq!(
        observed, 3,
        "visitation stops before the following declaration"
    );
    Ok(())
}
