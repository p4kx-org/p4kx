// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_extraction::cryxml::{self, XmlFieldKind, XmlRenderKind};
use p4kx_extraction::{SourceLocation, SourceUseRole, XmlRenderOrigins};
use p4kx_testkit::{CryXmlByteOrder, CryXmlSignature, TestElement};

use super::{Converted, XmlPreview};
use crate::XmlDocument;

type TestResult = Result<(), Box<dyn Error>>;

fn open(element: &TestElement, limit: usize) -> Result<XmlDocument, Box<dyn Error>> {
    let data = element.to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little);
    let (_preview, job) = XmlPreview::pending();
    match job.render(data, None, limit) {
        Some(Converted::Ready(document)) => Ok(document),
        _ => Err("the synthetic XML did not render".into()),
    }
}

fn proof(document: &XmlDocument) -> Result<&XmlRenderOrigins, Box<dyn Error>> {
    Ok(document
        .provenance()
        .ok_or("XML proof")?
        .map_err(ToString::to_string)?)
}

#[test]
fn a_cut_multiline_text_keeps_its_wholly_displayed_positioned_runs() -> TestResult {
    let element = TestElement::new("report").content("first & retained\nsecond cut\nthird");
    let input = element.to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little);
    let parsed = cryxml::parse(&input)?;
    let mut expected = Vec::new();
    parsed.write_xml(&mut expected)?;
    let expected = std::str::from_utf8(&expected)?;
    let limit = expected.find("second cut").ok_or("second text run")? + 3;
    let first_line = "first &amp; retained\n";
    let retained_end = expected.find(first_line).ok_or("first rendered run")? + first_line.len();
    let document = open(&element, limit)?;
    assert_eq!(
        document.bytes(),
        expected
            .as_bytes()
            .get(..retained_end)
            .ok_or("retained lines")?,
        "same complete lines"
    );
    assert!(document.is_truncated(), "partial next line is omitted");

    let proof = proof(&document)?;
    let mut text = proof
        .spans()
        .iter()
        .filter(|span| span.kind() == XmlRenderKind::Field(XmlFieldKind::Text));
    let retained = text
        .next()
        .ok_or("the retained part of the original multiline text")?;
    assert!(
        text.next().is_none(),
        "discarded text runs don't contribute"
    );
    let output = retained.output();
    assert_eq!(
        document
            .bytes()
            .get(usize::try_from(output.start)?..usize::try_from(output.end)?),
        Some(first_line.as_bytes()),
        "the retained range is the rendered text run"
    );
    assert_eq!(
        retained.origin().contains_encrypted_source(),
        None,
        "unbound input"
    );
    assert!(
        !retained.origin().source_set_complete(),
        "the original physical source is unbound"
    );
    let location = raw_text_location(&input, b"first & retained\n")?;
    assert!(
        proof.inventory().uses().iter().any(|used| {
            retained.origin().source_uses_refs().contains(used.id()) && used.location() == &location
        }),
        "only the exact original string bytes contribute"
    );
    Ok(())
}

fn raw_text_location(input: &[u8], contents: &[u8]) -> Result<SourceLocation, Box<dyn Error>> {
    let start = input
        .windows(contents.len())
        .position(|candidate| candidate == contents)
        .ok_or("the original binary string run")?;
    Ok(SourceLocation::archive_entry_decoded_bytes(
        u64::try_from(start)?..u64::try_from(start + contents.len())?,
        "stringBytes",
    ))
}

#[test]
fn an_authored_empty_attribute_has_a_real_zero_width_position() -> TestResult {
    let document = open(&TestElement::new("report").attribute("value", ""), 1024)?;
    assert!(!document.is_truncated(), "complete small XML rendering");
    let proof = proof(&document)?;
    let empty = proof
        .spans()
        .iter()
        .find(|span| span.kind() == XmlRenderKind::Field(XmlFieldKind::AttributeValue))
        .ok_or("the empty authored attribute")?;
    assert!(
        empty.output().is_empty(),
        "the empty output has an insertion point"
    );
    assert!(
        !empty.origin().source_uses_refs().is_empty(),
        "the original offset and terminator contribute"
    );
    let declaration = proof
        .spans()
        .iter()
        .find(|span| span.kind() == XmlRenderKind::Declaration)
        .ok_or("the rendered XML declaration")?;
    assert!(
        declaration.origin().source_uses_refs().is_empty(),
        "generated declaration"
    );
    assert_eq!(
        proof.contains_encrypted_source(),
        None,
        "generated syntax cannot establish original encryption"
    );
    Ok(())
}

#[test]
fn remapped_attribute_names_keep_the_renderers_exact_selectors() -> TestResult {
    let fixture = TestElement::new("report")
        .attribute("dup", "first")
        .attribute("dup", "second")
        .attribute("xmlns", "http://www.w3.org/XML/1998/namespace");
    let document = open(&fixture, 1024)?;
    assert_selector(
        &document,
        b"dup_2",
        "cryxmlAttributeNames",
        &[0, 0, 1],
        "suffixSelection",
    )?;
    assert_selector(
        &document,
        b"_xmlns",
        "cryxmlAttributeNames",
        &[0, 0, 3],
        "namespaceAliasSelection",
    )?;
    assert_selector(&document, b"_xmlns", "cryxmlAttribute", &[0, 2], "value")
}

fn assert_selector(
    document: &XmlDocument,
    rendered: &[u8],
    namespace: &str,
    indices: &[u64],
    field: &str,
) -> TestResult {
    let proof = proof(document)?;
    let rendered_start = document
        .bytes()
        .windows(rendered.len())
        .position(|bytes| bytes == rendered)
        .ok_or("the rendered attribute name")?;
    let output = u64::try_from(rendered_start)?..u64::try_from(rendered_start + rendered.len())?;
    let name = proof
        .spans()
        .iter()
        .find(|span| {
            span.kind() == XmlRenderKind::Field(XmlFieldKind::AttributeName)
                && span.output() == &output
        })
        .ok_or("the positioned remapped name")?;
    let expected = SourceLocation::Subresource {
        namespace: namespace.into(),
        indices: indices.to_vec(),
        field: field.into(),
    };
    assert!(
        proof.inventory().uses().iter().any(|used| {
            name.origin().source_uses_refs().contains(used.id())
                && used.role() == SourceUseRole::Selector
                && used.location() == &expected
        }),
        "the rendered name depends on the exact original selector {expected:?}"
    );
    Ok(())
}
