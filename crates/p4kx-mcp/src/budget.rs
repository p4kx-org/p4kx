// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};

use serde::Serialize;

use crate::error::ToolError;
use crate::page::Page;

/// The most bytes of JSON the items of one DataForge result may take.
pub(crate) const MAX_RESPONSE_LEN: usize = 256 << 10;

/// What is left of the response cap while a page is filled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Budget {
    left: usize,
}

impl Budget {
    /// Creates the budget of one response, [`MAX_RESPONSE_LEN`] bytes.
    pub(crate) const fn new() -> Self {
        Self {
            left: MAX_RESPONSE_LEN,
        }
    }

    /// Copies complete text after reserving its UTF-8 bytes.
    pub(crate) fn text(&mut self, value: &str) -> Result<String, ToolError> {
        self.admit_text(value.len())?;
        Ok(value.to_owned())
    }

    /// Reserves raw text bytes within the response budget.
    pub(crate) fn admit_text(&mut self, bytes: usize) -> Result<(), ToolError> {
        if self.take(bytes) {
            Ok(())
        } else {
            Err(ToolError::TooLargeItem {
                what: "response text".to_owned(),
                limit: MAX_RESPONSE_LEN,
                hint: "the complete text cannot fit in one response",
            })
        }
    }

    /// Returns the bytes left.
    pub(crate) const fn left(self) -> usize {
        self.left
    }

    /// Takes `len` bytes; returns `false` and takes nothing when they do not fit.
    pub(crate) const fn take(&mut self, len: usize) -> bool {
        if len > self.left {
            return false;
        }
        self.left -= len;
        true
    }
}

/// Fills one page from `items`, the items of a listing of `total` from the page's offset on:
/// at most the page size, and no more than fit [`MAX_RESPONSE_LEN`] as JSON together. Returns
/// them with the cursor of the next page; `describe` names an item that does not fit alone.
pub(crate) fn fill<T: Serialize>(
    page: Page,
    total: usize,
    items: impl IntoIterator<Item = Result<T, ToolError>>,
    describe: impl Fn(&T) -> String,
) -> Result<(Vec<T>, Option<String>), ToolError> {
    fill_within(page, total, 0, items, describe)
}

/// The room a cursor and the commas between items take in a response.
const ENVELOPE_SLACK: usize = 64;

/// Like [`fill`], for a result whose other fields take `reserved` bytes of JSON: the whole
/// response, not just the items, stays within [`MAX_RESPONSE_LEN`].
pub(crate) fn fill_within<T: Serialize>(
    page: Page,
    total: usize,
    reserved: usize,
    items: impl IntoIterator<Item = Result<T, ToolError>>,
    describe: impl Fn(&T) -> String,
) -> Result<(Vec<T>, Option<String>), ToolError> {
    let mut budget = Budget::new();
    if !budget.take(reserved.saturating_add(ENVELOPE_SLACK)) {
        return Err(ToolError::TooLargeItem {
            what: "result without its list".to_owned(),
            limit: MAX_RESPONSE_LEN,
            hint: "its other fields are too large for one response",
        });
    }
    let mut taken = Vec::new();
    for item in items.into_iter().take(page.limit()) {
        let item = match item {
            Ok(item) => item,
            Err(ToolError::TooLargeItem { .. }) if !taken.is_empty() => break,
            Err(error) => return Err(error),
        };
        if !budget.take(json_len(&item).saturating_add(1)) {
            if taken.is_empty() {
                return Err(ToolError::TooLargeItem {
                    what: describe(&item),
                    limit: MAX_RESPONSE_LEN,
                    hint: "ask for it by a narrower filter",
                });
            }
            break;
        }
        taken.push(item);
    }
    let next = page.next_cursor_after(taken.len(), total)?;
    Ok((taken, next))
}

/// Returns the length of `item` as JSON, without writing it anywhere.
pub(crate) fn json_len(item: &impl Serialize) -> usize {
    let mut counter = Counter::default();
    serde_json::to_writer(&mut counter, item).map_or(usize::MAX, |()| counter.len)
}

/// A sink that counts the bytes written to it.
#[derive(Debug, Default)]
pub(crate) struct Counter {
    len: usize,
}

impl Counter {
    /// Returns the bytes written so far.
    pub(crate) const fn len(&self) -> usize {
        self.len
    }
}

impl Write for Counter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.len = self.len.saturating_add(bytes.len());
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// A buffer that refuses to grow beyond its limit, so a large value stops being written as
/// soon as it cannot fit.
#[derive(Debug)]
pub(crate) struct Capped {
    bytes: Vec<u8>,
    limit: usize,
}

impl Capped {
    /// Creates an empty buffer of at most `limit` bytes.
    pub(crate) const fn new(limit: usize) -> Self {
        Self {
            bytes: Vec::new(),
            limit,
        }
    }

    /// Returns the bytes written.
    pub(crate) fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}

impl Write for Capped {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.limit.saturating_sub(self.bytes.len()) {
            return Err(io::Error::new(
                io::ErrorKind::FileTooLarge,
                "the response cap is reached",
            ));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::io::Write;

    use super::{Budget, Capped, MAX_RESPONSE_LEN, fill, json_len};
    use crate::error::ToolError;
    use crate::page::Page;

    fn texts(lens: &[usize]) -> Vec<Result<String, ToolError>> {
        lens.iter().map(|&len| Ok("a".repeat(len))).collect()
    }

    #[test]
    fn a_listing_page_ends_early_at_the_cap_and_continues_after_it() -> Result<(), Box<dyn Error>> {
        let page = Page::new(&"listing", None, Some(10))?;
        let (taken, next) = fill(page, 3, texts(&[100 << 10; 3]), String::clone)?;
        assert_eq!(taken.len(), 2, "two items fit the cap");
        let cursor = next.ok_or("a next page")?;
        let rest = Page::new(&"listing", Some(&cursor), Some(10))?;
        assert_eq!(
            rest.offset(),
            2,
            "the next page starts at the item left out"
        );
        Ok(())
    }

    #[test]
    fn a_listing_page_holds_at_most_the_page_size() -> Result<(), Box<dyn Error>> {
        let page = Page::new(&"listing", None, Some(2))?;
        let (taken, next) = fill(page, 3, texts(&[1, 1, 1]), String::clone)?;
        assert_eq!((taken.len(), next.is_some()), (2, true), "two and a cursor");
        Ok(())
    }

    #[test]
    fn a_listing_item_larger_than_the_cap_is_an_error() -> Result<(), Box<dyn Error>> {
        let page = Page::new(&"listing", None, None)?;
        let result = fill(page, 1, texts(&[MAX_RESPONSE_LEN]), |_| "item".to_owned());
        assert!(
            matches!(result, Err(ToolError::TooLargeItem { .. })),
            "too large alone"
        );
        Ok(())
    }

    #[test]
    fn the_budget_takes_what_fits_and_nothing_else() {
        let mut budget = Budget::new();
        assert!(budget.take(MAX_RESPONSE_LEN - 10), "most of it");
        assert!(!budget.take(11), "one byte too many");
        assert_eq!(budget.left(), 10, "nothing taken");
        assert!(budget.take(10), "the rest");
    }

    #[test]
    fn json_lengths_count_the_written_bytes() {
        assert_eq!(json_len(&"a\"b"), 6, "quotes and an escape");
        assert_eq!(json_len(&[1, 22]), 6, "an array");
    }

    #[test]
    fn a_capped_buffer_refuses_to_pass_its_limit() {
        let mut capped = Capped::new(4);
        assert!(capped.write_all(b"abc").is_ok(), "fits");
        assert!(capped.write_all(b"de").is_err(), "does not fit");
        assert_eq!(capped.into_bytes(), b"abc", "nothing of the refused write");
    }

    #[test]
    fn complete_text_admission_counts_utf8_and_keeps_refusals_atomic() -> Result<(), Box<dyn Error>>
    {
        let mut budget = Budget::new();
        assert!(
            budget.take(MAX_RESPONSE_LEN - 3),
            "leave exactly three raw bytes"
        );
        assert_eq!(budget.text("é")?, "é", "copy the complete two-byte scalar");
        assert_eq!(budget.left(), 1, "UTF-8 admission counts bytes");
        assert!(
            matches!(budget.text("xy"), Err(ToolError::TooLargeItem { .. })),
            "a refused text cannot become a clipped prefix"
        );
        assert_eq!(budget.left(), 1, "a refusal consumes no remaining budget");
        assert_eq!(budget.text("z")?, "z", "the remaining byte is usable");
        Ok(())
    }

    #[test]
    fn complete_text_admission_counts_each_output_occurrence() -> Result<(), Box<dyn Error>> {
        let mut budget = Budget::new();
        assert!(
            budget.take(MAX_RESPONSE_LEN - 4),
            "leave exactly four raw bytes"
        );
        assert_eq!(budget.text("abc")?, "abc", "the first occurrence fits");
        assert!(
            matches!(budget.text("abc"), Err(ToolError::TooLargeItem { .. })),
            "equal text in another output cell needs its own bytes"
        );
        assert_eq!(budget.left(), 1, "the second occurrence was not copied");
        Ok(())
    }
}

#[cfg(test)]
#[path = "budget_construction_tests.rs"]
mod construction_tests;
