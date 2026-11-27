// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, SourceUseRole, TagTree, TreeBuilder};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

use crate::CatalogError;
use crate::build::original_input::test_support::{self, TestResult};
use crate::record_header_field::RecordHeaderField;

fn bytes() -> Vec<u8> {
    const SOURCE_TYPE: &str = "UnselectedMissionDependencyFixture";
    DataForgeBuilder::new()
        .structure(TestStruct::new(SOURCE_TYPE).field("value", TestType::UInt32))
        .record(
            TestRecord::new(0x7112, SOURCE_TYPE, "Unselected", "entities/unselected.xml")
                .value("value", TestValue::UInt32(11)),
        )
        .build()
}

#[test]
fn an_unselected_record_registers_the_index_without_retaining_header_text() -> TestResult {
    test_support::with_input(&bytes(), |forge, input| {
        let record = forge.record(0).ok_or("actual excluded record")?;
        let mut trees = TreeBuilder::new();
        let before = input.origins().uses.freeze()?;
        assert!(
            super::read(
                &mut trees,
                &record,
                &TagTree::default(),
                Some(input),
                None,
                None
            )?
            .is_none(),
            "a readable unrelated record has no mission dependency outcome",
        );
        let after = input.origins().uses.freeze()?;
        let added: Vec<_> = after
            .uses()
            .iter()
            .filter(|usage| !before.uses().iter().any(|prior| prior.id() == usage.id()))
            .collect();
        let [index] = added.as_slice() else {
            return Err("the original index selector must be the sole new source use".into());
        };
        assert_eq!(index.role(), SourceUseRole::Selector, "index role");
        assert_eq!(
            index.location(),
            &RecordHeaderField::RecordIndex.location(record.index()),
            "the excluded actual record retains its original index selection",
        );
        let texts = input.origins().health_texts();
        for text in [record.record_path()?, record.type_name()?] {
            assert!(
                !texts.contains_text_for_test(text)?,
                "the excluded record's header has no retained text atom",
            );
        }
        Ok(())
    })
}

#[test]
fn cancellation_precedes_dependency_header_admission() -> TestResult {
    test_support::with_input(&bytes(), |forge, input| {
        let record = forge.record(0).ok_or("actual cancellation candidate")?;
        let mut trees = TreeBuilder::new();
        let before = input.origins().uses.freeze()?;
        let cancel = CancelToken::new();
        cancel.cancel();
        assert!(
            matches!(
                super::read(
                    &mut trees,
                    &record,
                    &TagTree::default(),
                    Some(input),
                    None,
                    Some(&cancel)
                ),
                Err(CatalogError::Cancelled)
            ),
            "the actual dependency reader preserves pre-cancelled execution",
        );
        assert_eq!(
            before,
            input.origins().uses.freeze()?,
            "cancellation does not admit header evidence",
        );
        Ok(())
    })
}
