// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::budget::{MAX_RESPONSE_LEN, fill_within};
use crate::error::ToolError;
use crate::page::Page;

fn kilobyte_items() -> impl Iterator<Item = String> {
    std::iter::repeat_with(|| "x".repeat(1000)).take(500)
}

fn described<T: AsRef<str>>(item: &T) -> String {
    format!("item of {} bytes", item.as_ref().len())
}

#[test]
fn the_envelope_takes_its_share_of_the_response_cap() -> Result<(), Box<dyn std::error::Error>> {
    let page = Page::new(&"audio", None, Some(1000))?;
    let (plain, _) = fill_within(page, 500, 0, kilobyte_items().map(Ok), described)?;
    let (crowded, next) = fill_within(
        page,
        500,
        MAX_RESPONSE_LEN - 100 * 1024,
        kilobyte_items().map(Ok),
        described,
    )?;
    assert!(
        plain.len() > 200,
        "{} items without an envelope",
        plain.len()
    );
    assert!(
        (80..=102).contains(&crowded.len()),
        "{} items beside a large envelope",
        crowded.len()
    );
    assert!(next.is_some(), "the rest comes on the next page");
    Ok(())
}

#[test]
fn an_envelope_alone_over_the_cap_is_a_clear_error() -> Result<(), Box<dyn std::error::Error>> {
    let page = Page::new(&"audio", None, None)?;
    let outcome = fill_within(
        page,
        500,
        MAX_RESPONSE_LEN,
        kilobyte_items().map(Ok),
        described,
    );
    assert!(
        matches!(outcome, Err(ToolError::TooLargeItem { .. })),
        "refused"
    );
    Ok(())
}
