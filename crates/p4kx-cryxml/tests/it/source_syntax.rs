// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_cryxml::{
    SourceContent, SourceContentPosition, SourceElementPosition, SourceSyntax, SourceText,
};

use p4kx_cryxml::{
    SourceAttributePosition, SourceDeclarationKind, SourceDocument, SourceReadOptions,
};

use p4kx_testkit::{CryXmlByteOrder, CryXmlSignature, TestElement};

type TestResult = Result<(), Box<dyn Error>>;

type ExpectedDeclaration = (
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    SourceDeclarationKind<'static>,
);

const EXPECTED_DECLARATIONS: [ExpectedDeclaration; 5] = [
    (
        "id",
        "00&1",
        "00&amp;1",
        "id = '00&amp;1'",
        SourceDeclarationKind::Attribute { namespace: None },
    ),
    (
        "xmlns:a",
        "urn:atl",
        "urn:atl",
        "xmlns:a='urn:atl'",
        SourceDeclarationKind::Namespace { prefix: Some("a") },
    ),
    (
        "a:Count",
        "001.50",
        "001.50",
        "a:Count='001.50'",
        SourceDeclarationKind::Attribute {
            namespace: Some("urn:atl"),
        },
    ),
    (
        "xmlns",
        "urn:default",
        "urn:default",
        "xmlns='urn:default'",
        SourceDeclarationKind::Namespace { prefix: None },
    ),
    (
        "count",
        "-00.25",
        "-00.25",
        "count='-00.25'",
        SourceDeclarationKind::Attribute { namespace: None },
    ),
];

#[test]
fn plain_xml_declares_every_authored_attribute_and_namespace_in_exact_order() -> TestResult {
    let text = "﻿<a:ATLConfig id = '00&amp;1' xmlns:a='urn:atl'                 a:Count='001.50' xmlns='urn:default' count='-00.25'/>";
    let source = SourceDocument::parse(text.as_bytes(), SourceReadOptions::default())?;
    let syntax = source.root().syntax()?;
    assert_eq!(
        syntax.qualified_name(),
        "a:ATLConfig",
        "original root QName"
    );
    let expected = EXPECTED_DECLARATIONS;
    let actual = syntax.declarations().collect::<Result<Vec<_>, _>>()?;
    assert_eq!(
        actual.len(),
        expected.len(),
        "each lexical declaration has one observation"
    );
    for (declaration, (name, value, literal, raw, kind)) in actual.iter().zip(expected) {
        let SourceAttributePosition::Text(range) = declaration.position() else {
            return Err("expected original text declaration coordinate".into());
        };
        assert_eq!(
            text.get(range.clone()),
            Some(raw),
            "exact original declaration with BOM offsets"
        );
        assert_eq!(declaration.qualified_name(), name, "lexical QName");
        assert_eq!(declaration.value(), value, "parsed declaration value");
        assert_eq!(
            declaration.lexeme(),
            Some(literal),
            "original value spelling"
        );
        assert_eq!(declaration.kind(), kind, "authored declaration category");
    }
    Ok(())
}

#[test]
fn plain_xml_content_keeps_ordered_text_comment_instruction_and_child_spans() -> TestResult {
    let text = "\u{feff}<a:ATLConfig xmlns:a='urn:atl'>left&amp;<![CDATA[<raw>]]><!--note--><?mode go?><a:Child/>right</a:ATLConfig >";
    let source = SourceDocument::parse(text.as_bytes(), SourceReadOptions::default())?;
    let syntax = source.root().syntax()?;
    let items = syntax.content()?.collect::<Result<Vec<_>, _>>()?;
    let [
        SourceContent::Text(first),
        SourceContent::Comment(comment),
        SourceContent::ProcessingInstruction {
            target,
            value,
            position,
            ..
        },
        SourceContent::Element(child),
        SourceContent::Text(last),
    ] = items.as_slice()
    else {
        return Err("mixed XML content was not observed in original order".into());
    };
    assert_eq!(
        (first.value(), comment.value(), last.value()),
        ("left&<raw>", "note", "right"),
        "decoded content facts"
    );
    check_text_span(first, text, "left&amp;<![CDATA[<raw>]]>")?;
    check_text_span(comment, text, "<!--note-->")?;
    check_text_span(last, text, "right")?;
    assert_eq!(
        (*target, *value),
        ("mode", Some("go")),
        "authored processing instruction"
    );
    assert_eq!(
        text.get(position.clone()),
        Some("<?mode go?>"),
        "exact instruction source extent"
    );
    let SourceElementPosition::Text(range) = child.position() else {
        return Err("original child coordinate".into());
    };
    assert_eq!(
        text.get(range),
        Some("<a:Child/>"),
        "exact child span inside mixed content"
    );
    assert_eq!(
        child.syntax()?.qualified_name(),
        "a:Child",
        "lexical child QName"
    );
    assert!(
        syntax.binary_text().is_none(),
        "plain mixed content has no CryXml content slot"
    );
    Ok(())
}

fn check_text_span(part: &SourceText<'_>, text: &str, raw: &str) -> TestResult {
    let SourceContentPosition::Text(range) = part.position() else {
        return Err("original text content coordinate".into());
    };
    assert_eq!(
        text.get(range.clone()),
        Some(raw),
        "original content source extent"
    );
    assert_eq!(part.lexeme(), Some(raw), "original content spelling");
    Ok(())
}

#[test]
fn binary_xml_keeps_duplicate_declarations_and_independent_original_content_slot() -> TestResult {
    let bytes = TestElement::new("ATLConfig")
        .attribute("root", "1")
        .child(
            TestElement::new("Future")
                .attribute("count", "001.50")
                .attribute("xmlns:a", "urn:binary")
                .attribute("count", "001.50")
                .content("binary text")
                .child(TestElement::new("First"))
                .child(TestElement::new("Second")),
        )
        .to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Big);
    let source = SourceDocument::parse(&bytes, SourceReadOptions::default())?;
    let child = source
        .root()
        .elements()
        .next()
        .ok_or("original CryXml child")?;
    let syntax = child.syntax()?;
    check_binary_declarations(&syntax)?;
    let text = syntax.binary_text().ok_or("original CryXml content slot")?;
    assert_eq!(
        (text.value(), text.lexeme()),
        ("binary text", None),
        "exact decoded content value"
    );
    assert_eq!(
        text.position(),
        &SourceContentPosition::CryXml(1),
        "content is owned by its physical node"
    );
    let items = syntax.content()?.collect::<Result<Vec<_>, _>>()?;
    let [
        SourceContent::Element(first),
        SourceContent::Element(second),
    ] = items.as_slice()
    else {
        return Err("CryXml child order".into());
    };
    assert_eq!(
        (first.name(), first.position()),
        ("First", SourceElementPosition::CryXml(2)),
        "first original child"
    );
    assert_eq!(
        (second.name(), second.position()),
        ("Second", SourceElementPosition::CryXml(3)),
        "second original child"
    );
    Ok(())
}

#[test]
fn document_content_keeps_original_prolog_and_epilog_occurrences() -> TestResult {
    let text = "\u{feff}<!--before--><?mode go?><ATLConfig/><!--after-->";
    let source = SourceDocument::parse(text.as_bytes(), SourceReadOptions::default())?;
    let items = source.content().collect::<Result<Vec<_>, _>>()?;
    let [
        SourceContent::Comment(first),
        SourceContent::ProcessingInstruction { position, .. },
        SourceContent::Element(root),
        SourceContent::Comment(last),
    ] = items.as_slice()
    else {
        return Err("expected four source-ordered document occurrences".into());
    };
    assert_eq!(
        (first.lexeme(), last.lexeme()),
        (Some("<!--before-->"), Some("<!--after-->")),
        "the source retains both comments independently"
    );
    assert_eq!(
        text.get(position.clone()),
        Some("<?mode go?>"),
        "the prolog instruction cites original bytes"
    );
    assert_eq!(
        root.name(),
        "ATLConfig",
        "the actual document element stays in order"
    );
    Ok(())
}

fn check_binary_declarations(syntax: &SourceSyntax<'_>) -> TestResult {
    let actual = syntax.declarations().collect::<Result<Vec<_>, _>>()?;
    assert_eq!(
        actual.len(),
        3,
        "duplicates and stored namespace declaration survive"
    );
    for (declaration, (index, name, value)) in actual.iter().zip([
        (1, "count", "001.50"),
        (2, "xmlns:a", "urn:binary"),
        (3, "count", "001.50"),
    ]) {
        assert_eq!(
            declaration.position(),
            &SourceAttributePosition::CryXml {
                node_index: 1,
                attribute_index: index,
            },
            "original physical declaration in the second node"
        );
        assert_eq!(
            (
                declaration.qualified_name(),
                declaration.value(),
                declaration.lexeme()
            ),
            (name, value, None),
            "stored binary declaration"
        );
    }
    Ok(())
}
