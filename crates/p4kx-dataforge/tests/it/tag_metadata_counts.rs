// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_dataforge::{DataForge, TagTree};

use crate::tag_metadata_source as source;

use source::TestResult;

#[test]
fn unknown_family_selections_have_their_own_exact_count() -> TestResult {
    for taxonomy in [false, true] {
        for malformed in [false, true] {
            let source = source::source(taxonomy, malformed)?;
            let forge = DataForge::open(&source.bytes)?;
            if malformed {
                source::unknown_original(&forge, &source)?;
            } else {
                source::other_original(&forge, &source)?;
            }
            let tree = TagTree::build(&forge);
            assert_eq!(
                tree.issues().unclassified(),
                usize::from(malformed),
                "one actual unreadable family selector"
            );
            assert_eq!(
                tree.issues().unreadable(),
                0,
                "no failed known-Tag canonical read is invented"
            );
            assert_eq!(
                tree.len(),
                usize::from(taxonomy),
                "known readable listed Tags are preserved"
            );
            assert_eq!(
                tree.issues().is_clean(),
                !malformed,
                "unknown membership stays separate from actual absence"
            );
        }
    }
    Ok(())
}
