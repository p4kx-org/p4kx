// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::convert::Infallible;
use std::error::Error;

use p4kx_model::{
    MaterialError, MaterialLibrary, MaterialReadError, MaterialSourceLimits, MaterialSourceLossKind,
};
use p4kx_testkit::{CryXmlByteOrder, CryXmlSignature, TestElement};

type TestResult = Result<(), Box<dyn Error>>;

fn read(
    bytes: &[u8],
    limits: MaterialSourceLimits,
) -> Result<MaterialLibrary, MaterialReadError<Infallible>> {
    MaterialLibrary::parse_with("synthetic/limits", bytes, limits, || Ok(()))
}

#[test]
fn an_empty_child_list_at_depth_zero_is_complete_but_an_actual_child_is_diagnosed() -> TestResult {
    let mut limits = MaterialSourceLimits::default();
    limits.depth = 0;
    let empty = read(b"<Material/>", limits)?;
    assert!(
        empty.authored.losses.is_empty(),
        "absence is not an exhausted frontier"
    );
    let populated = read(b"<Material><PublicParams Value='keep'/></Material>", limits)?;
    let loss = populated.authored.losses.first().ok_or("depth loss")?;
    assert_eq!(
        loss.kind,
        MaterialSourceLossKind::Depth,
        "actual populated frontier reaches the configured depth"
    );
    assert_eq!(
        loss.indices,
        [0],
        "first unretained original child is identified"
    );
    Ok(())
}

#[test]
fn element_and_text_bounds_are_explicit_and_never_compact_original_ordinals() -> TestResult {
    let mut limits = MaterialSourceLimits::default();
    limits.elements = 1;
    let elements = read(b"<Material><Unknown/></Material>", limits)?;
    assert!(
        elements
            .authored
            .losses
            .iter()
            .any(|loss| loss.kind == MaterialSourceLossKind::Elements),
        "element loss is explicit"
    );
    limits.elements = 100;
    limits.text_bytes = 1000;
    let text = format!(
        "<Material><Tiny/><Huge Value='{}'/><Last/></Material>",
        "x".repeat(2000)
    );
    let library = read(text.as_bytes(), limits)?;
    let children = &library.authored.root.as_ref().ok_or("root")?.children;
    let positions: Vec<_> = children
        .iter()
        .map(|child| child.authored.ordinal)
        .collect();
    assert_eq!(
        positions,
        [Some(0), Some(2)],
        "a skipped large node does not renumber a later retained child"
    );
    let loss = library
        .authored
        .losses
        .iter()
        .find(|loss| loss.kind == MaterialSourceLossKind::TextBytes)
        .ok_or("text loss")?;
    assert_eq!(
        loss.indices,
        [1],
        "oversized source node retains its actual location"
    );
    limits.text_bytes = 1;
    let omitted = read(b"<Material/>", limits)?;
    assert!(
        omitted.authored.root.is_none(),
        "a root that cannot fit is explicitly absent"
    );
    assert!(
        !omitted.authored.losses.is_empty(),
        "absence cannot be called complete"
    );
    Ok(())
}

#[test]
fn legacy_raw_depth_cut_is_reported_even_when_the_canonical_tree_retains_more() -> TestResult {
    let mut nested = TestElement::new("Leaf");
    for _ in 0..12 {
        nested = TestElement::new("Unknown").child(nested);
    }
    let xml = TestElement::new("Material").child(nested).to_xml();
    let library = MaterialLibrary::parse("synthetic/deep", xml.as_bytes())?;
    assert!(
        library.authored.losses.is_empty(),
        "canonical root fits its independent depth bound"
    );
    assert!(
        library
            .materials
            .first()
            .ok_or("material")?
            .truncated
            .iter()
            .any(|message| message.starts_with("raw nesting beyond ")),
        "existing limited raw field cannot silently lose deep children"
    );
    Ok(())
}

#[test]
fn plain_invalid_utf8_is_rejected_and_actual_binary_repairs_are_exposed() -> TestResult {
    let bad_text = b"<Material Name='bad\xff'/>";
    assert!(
        matches!(
            MaterialLibrary::parse("synthetic/invalid", bad_text),
            Err(MaterialError::TextEncoding(_))
        ),
        "plain invalid bytes are never silently replaced"
    );
    let element = TestElement::new("Material").attribute("Name", "repair-sentinel");
    let mut bytes = element.to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little);
    let marker = b"repair-sentinel";
    let offset = bytes
        .windows(marker.len())
        .position(|window| window == marker)
        .ok_or("synthetic marker")?;
    *bytes.get_mut(offset).ok_or("marker byte")? = 0xff;
    let document = p4kx_cryxml::parse(&bytes)?;
    assert!(
        document.is_lossy(),
        "the actual UTF8 validation failure sets the repair fact"
    );
    let library = MaterialLibrary::parse("synthetic/repaired", &bytes)?;
    assert!(
        library
            .authored
            .losses
            .iter()
            .any(|loss| loss.kind == MaterialSourceLossKind::RepairedEncoding),
        "binary repair means partial structured source projection"
    );
    Ok(())
}

#[test]
fn valid_binary_decoding_and_xml_escaping_do_not_claim_a_repair() -> TestResult {
    let element = TestElement::new("Material").attribute("Name", "valid & <quoted> \"value\"");
    let bytes = element.to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little);
    let document = p4kx_cryxml::parse(&bytes)?;
    assert!(
        !document.is_lossy(),
        "valid binary input is not partial merely because writing allocates"
    );
    let mut output = Vec::new();
    document.write_xml(&mut output)?;
    assert!(
        !document.is_lossy(),
        "XML escaping does not fabricate an input repair"
    );
    assert!(
        MaterialLibrary::parse("synthetic/valid", &bytes)?
            .authored
            .losses
            .is_empty(),
        "valid binary source stays complete"
    );
    Ok(())
}

#[test]
fn source_limit_and_checkpoint_interruption_are_distinct_typed_results() {
    let mut limits = MaterialSourceLimits::default();
    limits.source_bytes = 1;
    assert!(
        matches!(
            read(b"<Material/>", limits),
            Err(MaterialReadError::Material(
                MaterialError::SourceBytes { .. }
            ))
        ),
        "input size is bounded before structured parsing"
    );
    let mut calls = 0;
    let stopped = MaterialLibrary::parse_with(
        "synthetic/cancel",
        b"<Material><Unknown/></Material>",
        MaterialSourceLimits::default(),
        || {
            calls += 1;
            if calls == 5 {
                Err("original-stop")
            } else {
                Ok(())
            }
        },
    );
    assert!(
        matches!(
            stopped,
            Err(MaterialReadError::Interrupted("original-stop"))
        ),
        "checkpoint errors retain their original category and value"
    );
}
