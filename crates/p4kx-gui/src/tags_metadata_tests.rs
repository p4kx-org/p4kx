// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::sync::Arc;

use p4kx_extraction::{DataForge, DataForgeError, TagTree};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct};

use super::{State, TagsStatus, TagsTab, TagsView};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

fn tab(malformed: bool) -> TestResult<TagsTab> {
    let mut bytes = DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("MetadataTestFamily"))
        .record(TestRecord::new(
            1,
            "MetadataTestFamily",
            "Source",
            "source.xml",
        ))
        .build();
    if malformed {
        let name = b"MetadataTestFamily\0";
        let mut offsets = bytes
            .windows(name.len())
            .enumerate()
            .filter_map(|(offset, value)| (value == name).then_some(offset));
        let offset = offsets.next().ok_or("the actual type-name bytes")?;
        assert!(
            offsets.next().is_none(),
            "the fixture contains one exact type-name selector target"
        );
        *bytes.get_mut(offset).ok_or("the selected type-name byte")? = 0xff;
    }
    let forge = DataForge::open(&bytes)?;
    let record = forge.record(0).ok_or("the physical source Record")?;
    if malformed {
        assert!(
            matches!(
                record.type_name(),
                Err(DataForgeError::InvalidString { table: "blob", .. })
            ),
            "the original family selector genuinely fails before the convenience read"
        );
    } else {
        assert_eq!(
            record.type_name()?,
            "MetadataTestFamily",
            "the unrelated family is genuinely known"
        );
    }
    let tree = TagTree::build(&forge);
    assert!(tree.is_empty(), "the source contains no known Tag row");
    Ok(TagsTab {
        state: State::Ready(Box::new(TagsView::new(Arc::new(tree), 0))),
        ..TagsTab::default()
    })
}

#[test]
fn incomplete_empty_taxonomy_keeps_the_existing_ready_warning_route() -> TestResult {
    let tab = tab(true)?;
    assert!(
        matches!(tab.status(), TagsStatus::Ready(view) if view.irregularities() == 1),
        "an unknown family cannot become a clean no-tags status"
    );
    Ok(())
}

#[test]
fn known_unrelated_family_keeps_the_confirmed_no_tags_status() -> TestResult {
    let tab = tab(false)?;
    assert!(
        matches!(tab.status(), TagsStatus::NoTags),
        "known absence remains distinct from unreadable membership"
    );
    Ok(())
}
