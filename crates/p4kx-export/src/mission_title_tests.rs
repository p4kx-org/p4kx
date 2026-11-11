// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;

use super::{Token, render_key};
use crate::CatalogError;

type TestResult = Result<(), CatalogError>;

fn run(text: &str, table: &[(&str, &str)]) -> Result<String, CatalogError> {
    let mut localise = |key: &str| {
        Ok(table
            .iter()
            .find(|(name, _)| *name == key)
            .map(|(_, text)| Cow::Borrowed(*text)))
    };
    let mut resolve = |name: &str, _: Option<&str>| {
        Ok(table
            .iter()
            .find(|(token, _)| token.eq_ignore_ascii_case(name))
            .map(|(token, _)| Token::Key(token)))
    };
    render_key(
        "root",
        Cow::Borrowed(text),
        &mut localise,
        &mut resolve,
        None,
    )
}

#[test]
fn a_self_reference_keeps_the_bracket_at_its_active_edge() -> TestResult {
    assert_eq!(
        run("~mission(A)", &[("A", "x ~mission(A)")])?,
        "x [A]",
        "the exact cycle closure follows the first genuinely active key"
    );
    Ok(())
}

#[test]
fn the_same_cycle_closes_at_the_active_key_of_each_occurrence() -> TestResult {
    let original = [("A", "a~mission(B)"), ("B", "b~mission(a)")];
    assert_eq!(
        run("<~mission(A)|~mission(B)>", &original)?,
        "<ab[A]|ba[B]>",
        "each occurrence has its own cycle edge, even when the original graph is shared"
    );
    Ok(())
}

#[test]
fn high_fan_out_to_a_cycle_retains_each_original_occurrence() -> TestResult {
    let tokens = "~mission(A)".repeat(100);
    let output = run(&tokens, &[("A", &tokens)])?;
    assert_eq!(
        output,
        "[A]".repeat(10_000),
        "each original active edge keeps the bracket form"
    );
    Ok(())
}

#[test]
fn an_unaddressable_acyclic_render_fails_before_reserving_its_output() {
    let keys: Vec<_> = (0..=isize::BITS).map(|index| format!("K{index}")).collect();
    let localise = |key: &str| {
        let index: u32 = key.strip_prefix('K')?.parse().ok()?;
        if index == isize::BITS {
            return Some(Cow::Borrowed("x"));
        }
        let next = index + 1;
        Some(Cow::Owned(format!("~mission(K{next})~mission(K{next})")))
    };
    let mut resolve = |name: &str, _: Option<&str>| {
        Ok(keys
            .iter()
            .find(|key| *key == name)
            .map(|key| Token::Key(key.as_str())))
    };
    let mut localise = |key: &str| Ok(localise(key));
    assert!(
        matches!(
            render_key(
                "root",
                Cow::Borrowed("~mission(K0)"),
                &mut localise,
                &mut resolve,
                None
            ),
            Err(CatalogError::MissionTextLength)
        ),
        "a compact source can imply an unaddressable output"
    );
}

#[test]
fn an_unterminated_token_stays_as_written() -> TestResult {
    assert_eq!(
        run("a ~mission(B", &[("B", "b")])?,
        "a ~mission(B",
        "the original incomplete token stays literal"
    );
    Ok(())
}

#[test]
fn an_empty_token_becomes_empty_brackets() -> TestResult {
    assert_eq!(
        run("a ~mission() b", &[])?,
        "a [] b",
        "the established empty-token form"
    );
    Ok(())
}

#[test]
fn an_unknown_token_is_written_in_upper_case() -> TestResult {
    assert_eq!(
        run("~mission(cargo|x)", &[])?,
        "[CARGO]",
        "the established dangling-token form"
    );
    Ok(())
}

#[test]
fn a_nested_replacement_is_expanded() -> TestResult {
    let out = run("<~mission(A)>", &[("A", "in ~mission(B)"), ("B", "deep")])?;
    assert_eq!(out, "<in deep>", "the selected nested text stays in order");
    Ok(())
}
