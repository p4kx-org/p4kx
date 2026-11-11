// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_cryxml::{
    Format, Signature, SourceAttributePosition, SourceDocument, SourceElementPosition,
    SourceReadError, SourceReadOptions,
};
use p4kx_testkit::{CryXmlByteOrder, CryXmlSignature, TestElement};

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn original_text_ranges_keep_bom_namespace_and_unknown_literal_bytes() -> TestResult {
    let text = "\u{feff}\r\n<t:ATLConfig xmlns:t='urn:synthetic'><t:Future \
                xmlns:extra='urn:future' extra:literal = '00.25&amp;x' \
                Literal='Case' literal='001.50'>prefix<![CDATA[<raw>]]><t:Child/>suffix\
                </t:Future></t:ATLConfig>";
    let source = SourceDocument::parse(text.as_bytes(), SourceReadOptions::default())?;
    assert_eq!(source.format(), Format::Text, "plain original format");
    assert!(!source.is_lossy(), "valid source requires no string repair");
    let root = source.root();
    let element = root.elements().next().ok_or("original unknown child")?;
    assert_eq!(element.name(), "Future", "local element spelling");
    assert_eq!(element.namespace(), Some("urn:synthetic"), "inherited URI");
    assert_eq!(
        element.attribute("literal"),
        Some("00.25&x"),
        "bare-name lookup keeps the resolver's first local-name match"
    );
    let SourceElementPosition::Text(range) = element.position() else {
        return Err("original text element span".into());
    };
    assert_eq!(
        text.get(range),
        Some(
            "<t:Future xmlns:extra='urn:future' extra:literal = '00.25&amp;x' \
             Literal='Case' literal='001.50'>prefix<![CDATA[<raw>]]><t:Child/>suffix</t:Future>"
        ),
        "namespace declarations, mixed content and exact element spelling remain in the source span"
    );
    let mut attributes = element.attributes();
    let first = attributes.next().ok_or("original namespaced attribute")?;
    assert_eq!(first.name(), "literal", "text attribute's local spelling");
    assert_eq!(first.namespace(), Some("urn:future"), "expanded URI");
    assert_eq!(first.value(), "00.25&x", "parsed value decodes the entity");
    let SourceAttributePosition::Text(range) = first.position() else {
        return Err("original text attribute span".into());
    };
    assert_eq!(
        text.get(range.clone()),
        Some("extra:literal = '00.25&amp;x'"),
        "the byte span retains the authored QName, spacing, quotes and entity"
    );
    let rest: Vec<_> = attributes
        .map(|attribute| (attribute.name(), attribute.namespace(), attribute.value()))
        .collect();
    assert_eq!(
        rest,
        [("Literal", None, "Case"), ("literal", None, "001.50")],
        "case-distinct unqualified occurrences remain independently observable"
    );
    Ok(())
}

#[test]
fn whole_attribute_span_has_no_qname_or_separator_width_cutoff() -> TestResult {
    let name = "a".repeat(usize::from(u16::MAX) + 1);
    let spaces = " ".repeat(usize::from(u8::MAX) + 1);
    let authored = format!("{name}{spaces}={spaces}'001.50'");
    let text = format!("<ATLConfig {authored}/>");
    let source = SourceDocument::parse(text.as_bytes(), SourceReadOptions::default())?;
    let attribute = source.root().attributes().next().ok_or("long attribute")?;
    let SourceAttributePosition::Text(range) = attribute.position() else {
        return Err("text attribute coordinate".into());
    };
    assert_eq!(attribute.name(), name, "full original attribute name");
    assert_eq!(attribute.value(), "001.50", "original numeric literal");
    assert_eq!(
        text.get(range.clone()),
        Some(authored.as_str()),
        "whole source range remains exact across compact internal position fields"
    );
    Ok(())
}

#[test]
fn text_and_cryxml_borrow_the_same_ordered_element_and_attribute_facts() -> TestResult {
    let fixture = TestElement::new("ATLConfig")
        .attribute("atl_name", "Synthetic")
        .child(
            TestElement::new("AudioRTPCs")
                .child(TestElement::new("ATLRtpc").attribute("literal", "001.50"))
                .child(TestElement::new("ATLRtpc").attribute("literal", "-00.25")),
        );
    let text = fixture.to_xml();
    let binary = fixture.to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Big);
    let source_text = SourceDocument::parse(text.as_bytes(), SourceReadOptions::default())?;
    let source_binary = SourceDocument::parse(&binary, SourceReadOptions::default())?;
    let parsed = |source: &SourceDocument<'_>| -> TestResult {
        let root = source.root();
        assert_eq!(root.name(), "ATLConfig", "authored root");
        assert_eq!(root.attribute("atl_name"), Some("Synthetic"), "root name");
        let rtpcs = root.elements().next().ok_or("authored RTPC section")?;
        assert_eq!(rtpcs.name(), "AudioRTPCs", "actual source family");
        let elements: Vec<_> = rtpcs
            .elements()
            .map(|element| (element.name(), element.attribute("literal")))
            .collect();
        assert_eq!(
            elements,
            [("ATLRtpc", Some("001.50")), ("ATLRtpc", Some("-00.25"))],
            "same authored declaration facts and literal spellings in both source formats"
        );
        Ok(())
    };
    parsed(&source_text)?;
    parsed(&source_binary)?;
    assert_eq!(
        source_binary.format(),
        Format::CryXml(Signature::CryXmlB),
        "original binary signature"
    );
    Ok(())
}

#[test]
fn cryxml_duplicate_attributes_keep_distinct_physical_table_coordinates() -> TestResult {
    let binary = TestElement::new("ATLConfig")
        .attribute("header", "synthetic")
        .child(
            TestElement::new("Future")
                .attribute("literal", "first")
                .attribute("Literal", "case")
                .attribute("literal", "last"),
        )
        .to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little);
    let source = SourceDocument::parse(&binary, SourceReadOptions::default())?;
    let child = source.root().elements().next().ok_or("binary child")?;
    assert_eq!(
        child.position(),
        SourceElementPosition::CryXml(1),
        "physical node table index"
    );
    assert_eq!(
        child.attribute("literal"),
        Some("first"),
        "first authored match"
    );
    let actual: Vec<_> = child.attributes().collect();
    assert_eq!(
        actual.len(),
        3,
        "all duplicate binary declarations retained"
    );
    for (attribute, (index, name, value)) in actual.iter().zip([
        (1, "literal", "first"),
        (2, "Literal", "case"),
        (3, "literal", "last"),
    ]) {
        let SourceAttributePosition::CryXml {
            node_index,
            attribute_index,
            ..
        } = attribute.position()
        else {
            return Err("binary attribute coordinate".into());
        };
        assert_eq!(
            (*node_index, *attribute_index),
            (1, index),
            "original binary slot"
        );
        assert_eq!(
            (attribute.name(), attribute.value()),
            (name, value),
            "stored value"
        );
    }
    Ok(())
}

#[test]
fn complete_source_can_traverse_beyond_legacy_open_depth() -> TestResult {
    const DEPTH: usize = 1_024;
    let text = format!(
        "{}<terminal/>{}",
        "<open>".repeat(DEPTH),
        "</open>".repeat(DEPTH)
    );
    let legacy =
        SourceReadOptions::default().with_open_text_depth_limit(p4kx_cryxml::MAX_TEXT_DEPTH);
    assert!(
        matches!(
            SourceDocument::parse(text.as_bytes(), legacy),
            Err(SourceReadError::TextDepth(_))
        ),
        "the explicit Assembly preflight still rejects the legacy depth overflow"
    );
    let source = SourceDocument::parse(text.as_bytes(), SourceReadOptions::default())?;
    let mut element = source.root();
    for _ in 0..DEPTH {
        element = element
            .elements()
            .next()
            .ok_or("complete nested traversal")?;
    }
    assert_eq!(
        element.name(),
        "terminal",
        "every deep source element stays reachable"
    );
    Ok(())
}

#[test]
fn text_node_limit_and_malformed_sources_return_typed_errors() {
    let limited = SourceReadOptions::default().with_text_node_limit(2);
    assert!(
        matches!(
            SourceDocument::parse(b"<ATLConfig><Future/></ATLConfig>", limited),
            Err(SourceReadError::Text(roxmltree::Error::NodesLimitReached))
        ),
        "an explicit node budget refuses the source instead of returning a shortened tree"
    );
    assert!(
        matches!(
            SourceDocument::parse(b"<ATLConfig><Future/>", SourceReadOptions::default()),
            Err(SourceReadError::Text(_))
        ),
        "an unclosed text root is a parse failure"
    );
    assert!(
        matches!(
            SourceDocument::parse(b"<ATLConfig>\xff</ATLConfig>", SourceReadOptions::default()),
            Err(SourceReadError::Utf8(_))
        ),
        "malformed text UTF-8 retains its error category"
    );
    assert!(
        matches!(
            SourceDocument::parse(b"CryXmlB\0", SourceReadOptions::default()),
            Err(SourceReadError::CryXml(_))
        ),
        "a truncated original binary input retains its format error"
    );
}
