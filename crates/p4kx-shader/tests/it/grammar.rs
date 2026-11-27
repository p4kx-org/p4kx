// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_shader::{Encoding, Field, Form, Limits, PropertyFieldKind, RootFieldKind};

use super::support::{self, TestResult};

const SOURCE: &str = r#"// synthetic declarations, not game source
Version(1.2)
Version = later
UsesCommonGlobalFlags
Property
{
    Name = %SYNTHETIC
    Mask = 0x20000000000001 // larger than a JSON safe integer
    Property("Editor { label }")
    Description("Quoted \" ) } text")
    DependencySet = ($A | $B)
    DependencySet($C)
    DependencyReset = $D
    Hidden
    Hidden =
    Mystery { Nested("( })") }
}
Property { Name(); Mask(0); }
Unknown = (call({ child }))
"#;

#[test]
fn ordered_views_keep_all_known_fields_duplicates_and_unknown_blocks() -> TestResult {
    let definition = p4kx_shader::read(SOURCE.as_bytes(), Limits::default())?;
    assert_eq!(
        definition.source(),
        SOURCE.as_bytes(),
        "raw source is authoritative"
    );
    assert_eq!(
        definition.fields_of(RootFieldKind::Version).count(),
        2,
        "versions are not last-wins"
    );
    assert_eq!(
        definition
            .fields_of(RootFieldKind::UsesCommonGlobalFlags)
            .count(),
        1,
        "bare common flag remains present"
    );
    let properties: Vec<_> = definition.properties().collect();
    assert_eq!(
        properties.len(),
        2,
        "each source Property remains a distinct declaration"
    );
    let first = properties.first().ok_or("first Property")?;
    let kinds: Vec<_> = first.fields().map(Field::property_kind).collect();
    assert_eq!(
        kinds,
        [
            Some(PropertyFieldKind::Name),
            Some(PropertyFieldKind::Mask),
            Some(PropertyFieldKind::Label),
            Some(PropertyFieldKind::Description),
            Some(PropertyFieldKind::DependencySet),
            Some(PropertyFieldKind::DependencySet),
            Some(PropertyFieldKind::DependencyReset),
            Some(PropertyFieldKind::Hidden),
            Some(PropertyFieldKind::Hidden),
            None,
        ],
        "unknown fields and duplicate dependencies retain source order"
    );
    let mystery = first.fields().last().ok_or("unknown block")?;
    assert_eq!(
        mystery.name(),
        Some("Mystery"),
        "unknown name is not erased"
    );
    assert_eq!(
        mystery.declaration().children.len(),
        1,
        "unknown nesting remains structural"
    );
    assert_eq!(
        mystery.raw(),
        Some("Mystery { Nested(\"( })\") }"),
        "quotes protect structural punctuation"
    );
    Ok(())
}

#[test]
fn marker_absent_empty_and_explicit_zero_remain_different() -> TestResult {
    let definition = p4kx_shader::read(
        b"Property { Hidden; Hidden(); Mask(0); Name = ; }",
        Limits::default(),
    )?;
    let property = definition.properties().next().ok_or("Property")?;
    assert_eq!(
        property.fields_of(PropertyFieldKind::Description).count(),
        0,
        "missing field is not fabricated"
    );
    let hidden: Vec<_> = property.fields_of(PropertyFieldKind::Hidden).collect();
    let bare = hidden.first().ok_or("bare Hidden")?;
    let empty = hidden.get(1).ok_or("empty Hidden")?;
    assert_eq!(
        (bare.declaration().form, bare.value()),
        (Form::Marker, None),
        "bare marker has no value"
    );
    assert_eq!(
        (empty.declaration().form, empty.value()),
        (Form::Parentheses, Some("")),
        "explicit empty parentheses stay present"
    );
    assert_eq!(
        support::first_field(&definition, PropertyFieldKind::Mask)?
            .mask()
            .interpreted,
        Ok(0),
        "zero is an interpreted value"
    );
    assert_eq!(
        support::first_field(&definition, PropertyFieldKind::Name)?.value(),
        Some(" "),
        "equals preserves authored whitespace"
    );
    Ok(())
}

#[test]
fn utf8_marker_and_multibyte_text_keep_original_byte_offsets() -> TestResult {
    let source = "\u{feff}Version(工具)\r\nProperty { Name = %Ä; }";
    let definition = p4kx_shader::read(source.as_bytes(), Limits::default())?;
    assert_eq!(
        definition.encoding(),
        Encoding::Utf8Bom,
        "BOM is reported, not removed from source"
    );
    let version = definition
        .fields_of(RootFieldKind::Version)
        .next()
        .ok_or("version")?;
    assert_eq!(
        version.declaration().span.start,
        3,
        "offsets include original BOM bytes"
    );
    assert_eq!(
        version.value(),
        Some("工具"),
        "multibyte value remains exact"
    );
    let name = support::first_field(&definition, PropertyFieldKind::Name)?;
    let span = name.declaration().literal.ok_or("name literal span")?;
    assert_eq!(
        span.bytes(definition.source()),
        Some("%Ä".as_bytes()),
        "ranges address uncompressed original bytes"
    );
    assert!(
        span.text("x").is_none(),
        "foreign out-of-range slices fail safely"
    );
    Ok(())
}

#[test]
fn comments_unknown_scopes_and_case_variants_are_never_collapsed() -> TestResult {
    let source = b"/* } ( \" */ VERSION(1)\nProperty{ Name=%A; name=%B; }\nUnknown { Property{ Hidden } }\n'x'";
    let definition = p4kx_shader::read(source, Limits::default())?;
    assert_eq!(
        definition.source(),
        source,
        "comments and unknown quoted forms remain raw"
    );
    assert_eq!(
        definition.fields_of(RootFieldKind::Version).count(),
        1,
        "known keyword matching is case insensitive"
    );
    assert_eq!(
        definition.properties().count(),
        1,
        "unknown scope does not fabricate top-level semantics"
    );
    let property = definition.properties().next().ok_or("Property")?;
    let names: Vec<_> = property
        .fields_of(PropertyFieldKind::Name)
        .map(Field::literal)
        .collect();
    assert_eq!(
        names,
        [Some("%A"), Some("%B")],
        "case variants retain separate original fields"
    );
    assert_eq!(
        definition
            .declarations()
            .last()
            .ok_or("unknown quoted node")?
            .form,
        Form::Unknown,
        "unknown syntax remains represented"
    );
    Ok(())
}

#[test]
fn unquoted_description_contractions_preserve_their_original_text() -> TestResult {
    let source = br#"Property {
        Description(pilot's controls)
        Description("pilot's controls (safe)")
        Description('quoted (value)')
        Description(pilot's)
    }"#;
    let definition = p4kx_shader::read(source, Limits::default())?;
    let property = definition.properties().next().ok_or("original Property")?;
    let descriptions: Vec<_> = property.fields_of(PropertyFieldKind::Description).collect();
    let [unquoted, double, single, word] = descriptions.as_slice() else {
        return Err("the four original descriptions must survive".into());
    };
    assert_eq!(
        definition.source(),
        source,
        "the source bytes are unchanged"
    );
    assert_eq!(
        unquoted.value(),
        Some("pilot's controls"),
        "the intra-word apostrophe is literal"
    );
    assert_eq!(
        double.literal(),
        Some("\"pilot's controls (safe)\""),
        "double quotes still protect punctuation"
    );
    assert_eq!(
        single.literal(),
        Some("'quoted (value)'"),
        "a standalone single-quoted value remains quoted"
    );
    assert_eq!(
        word.literal(),
        Some("pilot's"),
        "a contraction remains one unquoted atom"
    );
    Ok(())
}
