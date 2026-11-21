// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fmt::Write as _;

use p4kx_model::{MaterialLibrary, MaterialSourceLossKind, RawElement};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;
const CHILDREN: usize = 4096;
const TOTAL_ELEMENTS: usize = 4097;

#[test]
fn material_authored_complete_retains_all_4097_original_source_elements() -> TestResult {
    let xml = original_xml()?;
    assert_original(&xml)?;
    let library = MaterialLibrary::parse("synthetic/complete-authored-elements", xml.as_bytes())?;
    assert_material_fragments(&library)?;
    assert!(
        library
            .authored
            .losses
            .iter()
            .all(|loss| matches!(loss.kind, MaterialSourceLossKind::Elements)),
        "neither owned-text, depth nor encoding loss explains this element-only fixture"
    );
    let root = library
        .authored
        .root
        .as_ref()
        .ok_or("original authored root")?;
    assert_eq!(root.name, "Material", "actual original root");
    assert_eq!(
        root.children.len(),
        CHILDREN,
        "all4096 original children survive the4097-total-element source"
    );
    for (child, ordinal) in root.children.iter().zip(0..CHILDREN) {
        assert_fragment(child, ordinal);
        assert_eq!(
            child.authored.ordinal,
            Some(ordinal),
            "original source ordinal independent of retained count"
        );
    }
    assert!(
        library.authored.losses.is_empty(),
        "the complete source projection has no omitted original element"
    );
    Ok(())
}

fn original_xml() -> TestResult<String> {
    let mut xml = String::from("<Material>");
    for ordinal in 0..CHILDREN {
        write!(xml, "<Probe Ordinal=\"{ordinal}\"/>")?;
    }
    xml.push_str("</Material>");
    Ok(xml)
}

fn assert_original(xml: &str) -> TestResult {
    let document = roxmltree::Document::parse(xml)?;
    let root = document.root_element();
    assert_eq!(
        root.tag_name().name(),
        "Material",
        "actual original XML root"
    );
    let elements = document
        .descendants()
        .filter(roxmltree::Node::is_element)
        .count();
    assert_eq!(
        elements, TOTAL_ELEMENTS,
        "all4097 actual original element occurrences before material parsing"
    );
    let children: Vec<_> = root
        .children()
        .filter(roxmltree::Node::is_element)
        .collect();
    assert_eq!(children.len(), CHILDREN, "all4096 direct original children");
    for (child, ordinal) in children.iter().zip(0..CHILDREN) {
        assert_eq!(
            child.tag_name().name(),
            "Probe",
            "uninterpreted original element name"
        );
        assert_eq!(
            child.attribute("Ordinal"),
            Some(ordinal.to_string().as_str()),
            "every exact source token in authored order"
        );
        assert!(
            !child.children().any(|node| node.is_element()),
            "fixture depth is only root plus one child"
        );
    }
    let last = children.last().ok_or("actual last original element")?;
    assert_eq!(
        last.attribute("Ordinal"),
        Some("4095"),
        "independent original tail sentinel"
    );
    Ok(())
}

fn assert_material_fragments(library: &MaterialLibrary) -> TestResult {
    assert!(library.single, "no submaterial-count policy participates");
    assert_eq!(library.materials.len(), 1, "single interpreted material");
    let material = library
        .materials
        .first()
        .ok_or("actual interpreted material")?;
    assert_eq!(
        material.elements.len(),
        CHILDREN,
        "separate per-material raw fragment budget retains the exact4096 leaves"
    );
    assert!(
        material.truncated.is_empty(),
        "no raw4096/depth8 truncation explains the authored omission"
    );
    for (fragment, ordinal) in material.elements.iter().zip(0..CHILDREN) {
        assert_fragment(fragment, ordinal);
    }
    Ok(())
}

fn assert_fragment(fragment: &RawElement, ordinal: usize) {
    assert_eq!(fragment.name, "Probe", "exact original uninterpreted name");
    assert_eq!(
        fragment.attributes,
        [("Ordinal".to_owned(), ordinal.to_string())],
        "exact original attribute token"
    );
    assert!(
        fragment.children.is_empty(),
        "no nested source was introduced"
    );
}

#[path = "material_authored_element_policy.rs"]
mod element_policy;
