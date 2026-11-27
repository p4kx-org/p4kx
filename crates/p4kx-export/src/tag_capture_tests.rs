// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, TagTree};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

use super::{TagConsumers, TagDependencies, read};
use crate::Kind;
use crate::build::original_input::OriginalCatalogInput;
use crate::build::original_input::test_support::{self, TestResult};
use crate::export_diagnostic::ExportDiagnosticCode;
use crate::export_ledger::SourceDisposition;
use crate::export_tag_read;

const ROOT: u128 = 1;
const LEAF: u128 = 2;
const MISSING: u128 = 3;
const DATABASE: u128 = 4;

fn bytes() -> Vec<u8> {
    DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new("Tag")
                .field("tagName", TestType::String)
                .field("legacyGUID", TestType::UInt32)
                .array("children", TestType::ReferenceTo("Tag".to_owned())),
        )
        .structure(
            TestStruct::new("TagDatabase").array("tags", TestType::ReferenceTo("Tag".to_owned())),
        )
        .record(tag(ROOT, "Root", &[LEAF, MISSING, LEAF]))
        .record(tag(LEAF, "Leaf", &[]))
        .record(tag(LEAF, "Shadow", &[]))
        .record(
            TestRecord::new(
                DATABASE,
                "TagDatabase",
                "TagDatabase.Roots",
                "tags/roots.xml",
            )
            .value("tags", TestValue::Array(vec![TestValue::Reference(ROOT)])),
        )
        .build()
}

fn tag(id: u128, name: &str, children: &[u128]) -> TestRecord {
    TestRecord::new(id, "Tag", format!("Tag.{name}"), format!("tags/{name}.xml"))
        .value("tagName", TestValue::text(name))
        .value("legacyGUID", TestValue::UInt32(0))
        .value(
            "children",
            TestValue::Array(children.iter().copied().map(TestValue::Reference).collect()),
        )
}

#[test]
fn canonical_tag_inspection_serves_independent_consumers() -> TestResult {
    test_support::with_input(&bytes(), |forge, input| {
        for (contracts, paints) in [(true, true), (false, true), (true, false), (false, false)] {
            let tags = read(forge, input, None, TagConsumers { contracts, paints })?;
            assert_eq!(
                tags.ledger.field_inspections.len(),
                4,
                "the four physical source bodies each have one whole-record inspection"
            );
            assert_eq!(
                tags.dependencies.is_some(),
                contracts,
                "contract observations follow the caller selection"
            );
            assert_eq!(
                tags.paint_dependencies.is_some(),
                paints,
                "paint observations follow the caller selection"
            );
            for (kind, dependencies) in [
                (Kind::Contracts, tags.dependencies),
                (Kind::Paints, tags.paint_dependencies),
            ] {
                if let Some(dependencies) = dependencies {
                    compare_consumer(forge, input, kind, tags.context.tree(), &dependencies)?;
                }
            }
        }
        Ok(())
    })
}

fn compare_consumer<'source>(
    forge: &'source DataForge<'source>,
    input: &OriginalCatalogInput<'_, 'source>,
    kind: Kind,
    shared_tree: &TagTree,
    dependencies: &TagDependencies<'source>,
) -> TestResult {
    assert!(
        std::ptr::eq(dependencies.tree.tree(), shared_tree),
        "the consumer resolves tags using the catalog's exact tree"
    );
    assert!(
        dependencies.ledger.field_inspections.is_empty(),
        "one canonical inspection serves both outputs"
    );
    let expected = export_tag_read::read(forge, kind, Some(input), None)?;
    assert_eq!(
        dependencies.failures, expected.failures,
        "all original tag failures stay with the consumer"
    );
    assert_eq!(
        dependencies.ledger.sources, expected.ledger.sources,
        "the same physical reads and skips count against the consumer"
    );
    assert_eq!(
        dependencies.ledger.field_diagnostics, expected.ledger.field_diagnostics,
        "the same original relation findings count against the consumer"
    );
    assert!(
        dependencies
            .ledger
            .sources
            .iter()
            .any(|source| source.disposition == SourceDisposition::SkippedAfterRead),
        "a duplicate physical Tag contributes the original skip outcome"
    );
    assert!(
        dependencies
            .ledger
            .field_diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == ExportDiagnosticCode::DanglingReference),
        "the original dangling child occurrence produces a dependency diagnostic"
    );
    for diagnostic in &dependencies.ledger.field_diagnostics {
        for example in &diagnostic.examples {
            example.body()?.check_record_owner(example.subject())?;
        }
    }
    Ok(())
}
