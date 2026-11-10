// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::Cell;
use std::error::Error;

use super::{MAX_RESPONSE_LEN, fill};
use crate::error::ToolError;
use crate::page::Page;

fn constructed_oversize() -> ToolError {
    ToolError::TooLargeItem {
        what: "constructed item".to_owned(),
        limit: MAX_RESPONSE_LEN,
        hint: "item does not fit",
    }
}

#[test]
fn constructed_item_oversize_continues_at_the_exact_next_cursor() -> Result<(), Box<dyn Error>> {
    let page = Page::new(&"construction", None, None)?;
    let visited = Cell::new(0);
    let items = [
        Ok("first".to_owned()),
        Err(constructed_oversize()),
        Ok("last".to_owned()),
    ]
    .into_iter()
    .inspect(|_| visited.set(visited.get() + 1));
    let (kept, cursor) = fill(page, 3, items, String::clone)?;
    assert_eq!(
        kept,
        ["first"],
        "only the complete original prefix is retained"
    );
    assert_eq!(
        visited.get(),
        2,
        "the refused item is not skipped to a later item"
    );
    let cursor = cursor.ok_or("the refused item's continuation")?;
    let next = Page::new(&"construction", Some(&cursor), None)?;
    assert_eq!(next.offset(), 1, "cursor retries the exact refused item");
    let outcome = fill::<String>(next, 3, [Err(constructed_oversize())], String::clone);
    assert!(
        matches!(outcome, Err(ToolError::TooLargeItem { .. })),
        "the same oversize item is an explicit first-item refusal on continuation"
    );
    Ok(())
}

#[test]
fn constructed_item_oversize_is_fatal_as_the_first_item() -> Result<(), Box<dyn Error>> {
    let page = Page::new(&"construction", None, None)?;
    let result = fill::<String>(page, 1, [Err(constructed_oversize())], String::clone);
    assert!(
        matches!(result, Err(ToolError::TooLargeItem { .. })),
        "an empty successful page cannot hide the first refused item"
    );
    Ok(())
}

#[test]
fn constructed_item_other_failure_is_fatal_after_a_prefix() -> Result<(), Box<dyn Error>> {
    let page = Page::new(&"construction", None, None)?;
    let items = [
        Ok("first".to_owned()),
        Err(ToolError::TooLong {
            name: "path",
            max: 1024,
        }),
    ];
    let result = fill(page, 2, items, String::clone);
    assert!(
        matches!(
            result,
            Err(ToolError::TooLong {
                name: "path",
                max: 1024
            })
        ),
        "an unrelated failure remains fatal after complete items"
    );
    Ok(())
}
