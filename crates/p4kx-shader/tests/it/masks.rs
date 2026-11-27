// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_shader::{Field, Limits, MaskIssue, PropertyFieldKind};

use super::support::{self, TestResult};

#[test]
fn integer_words_are_exact_and_unsupported_values_never_become_zero() -> TestResult {
    let cases = [
        ("0", Ok(0)),
        ("0x0", Ok(0)),
        ("0XfF", Ok(255)),
        ("0x20000000000001", Ok(9_007_199_254_740_993)),
        ("0xffffffffffffffff", Ok(u64::MAX)),
        ("0x10000000000000000", Err(MaskIssue::Overflow)),
        ("0x", Err(MaskIssue::Malformed)),
        ("0xG1", Err(MaskIssue::Malformed)),
        ("0x+1", Err(MaskIssue::Malformed)),
        ("17", Err(MaskIssue::UnknownRadix)),
        ("0001", Err(MaskIssue::UnknownRadix)),
        ("-1", Err(MaskIssue::UnsupportedExpression)),
        ("0x1 | 0x2", Err(MaskIssue::UnsupportedExpression)),
        ("($A | $B)", Err(MaskIssue::UnsupportedExpression)),
        ("\"0x1\"", Err(MaskIssue::UnsupportedExpression)),
    ];
    for (raw, expected) in cases {
        let source = format!("Property {{ Mask = {raw}; }}");
        let definition = p4kx_shader::read(source.as_bytes(), Limits::default())?;
        let mask = support::first_field(&definition, PropertyFieldKind::Mask)?.mask();
        assert_eq!(
            mask.raw,
            Some(format!(" {raw}").as_str()),
            "authored value spelling survives: {raw}"
        );
        assert_eq!(
            mask.interpreted, expected,
            "numeric interpretation must be explicit: {raw}"
        );
    }
    Ok(())
}

#[test]
fn absent_empty_bare_and_duplicate_masks_have_distinct_states() -> TestResult {
    let definition = p4kx_shader::read(
        b"Property{ Mask; Mask(); Mask = ; Mask(0); Mask(0x2); } Property{}",
        Limits::default(),
    )?;
    let mut properties = definition.properties();
    let first = properties.next().ok_or("first Property")?;
    let values: Vec<_> = first
        .fields_of(PropertyFieldKind::Mask)
        .map(|field| field.mask().interpreted)
        .collect();
    assert_eq!(
        values,
        [
            Err(MaskIssue::MissingValue),
            Err(MaskIssue::EmptyValue),
            Err(MaskIssue::EmptyValue),
            Ok(0),
            Ok(2)
        ],
        "each authored occurrence remains separately interpretable"
    );
    let absent = properties.next().ok_or("second Property")?;
    assert_eq!(
        absent.fields_of(PropertyFieldKind::Mask).count(),
        0,
        "absent mask does not acquire zero or an empty declaration"
    );
    Ok(())
}

#[test]
fn comments_do_not_change_a_sole_numeric_token_and_expressions_stay_raw() -> TestResult {
    let definition = p4kx_shader::read(
        b"Property { Mask( /* ignore } */ 0x2 /* ignore ( */ ); Mask = 0x4 // }\n}",
        Limits::default(),
    )?;
    let property = definition.properties().next().ok_or("Property")?;
    let masks: Vec<_> = property
        .fields_of(PropertyFieldKind::Mask)
        .map(Field::mask)
        .collect();
    assert_eq!(
        masks.first().ok_or("first mask")?.interpreted,
        Ok(2),
        "comments are lexical trivia"
    );
    assert_eq!(
        masks.get(1).ok_or("second mask")?.interpreted,
        Ok(4),
        "line comments do not become an expression"
    );
    assert_eq!(
        masks.first().ok_or("raw mask")?.raw,
        Some(" /* ignore } */ 0x2 /* ignore ( */ "),
        "raw value retains comment bytes independently"
    );
    Ok(())
}
