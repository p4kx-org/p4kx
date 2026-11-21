// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use p4kx_export::{CatalogOptions, ExportHealthStatus, Kind};
use p4kx_extraction::{InputScope, SourceInventory, SourceLocation, SourceUseRole};
use serde_json::{Value, json};

use crate::health_origin_fixture::{Input, TestResult};
use crate::loot_reference_v3_fixture as fixture;
use crate::loot_reference_v3_source;
use crate::loot_v3_graph::{self as graph, Graph};
use crate::loot_v3_origin::{self as origin, ExpectedOrigin, ExpectedUse, SourceSetProblem};

pub(crate) struct Output<'catalog> {
    pub(crate) catalog: &'catalog Value,
    pub(crate) inventory: &'catalog SourceInventory,
    pub(crate) input: Input,
}

impl<'catalog> Output<'catalog> {
    pub(crate) fn node(&self, field: &str) -> TestResult<&'catalog Value> {
        Graph::new(self.catalog, fixture::OWNER)?.root(field)
    }

    pub(crate) fn reference(&self, field: &str, kind: &str) -> TestResult<&'catalog Value> {
        graph::payload(self.node(field)?, kind)?
            .get("value")
            .ok_or_else(|| "typed original reference payload".into())
    }

    pub(crate) fn header(&self, record: u32, member: &str) -> TestResult<&'catalog Value> {
        self.row(record)?
            .get(member)
            .ok_or_else(|| format!("actual copied header {member}").into())
    }

    pub(crate) fn count(&self, record: u32, expected: usize) -> TestResult {
        assert_eq!(
            self.row(record)?
                .get("values")
                .and_then(Value::as_array)
                .map(Vec::len),
            Some(expected),
            "all independently counted original occurrences, no target-body expansion or ghost node"
        );
        Ok(())
    }

    fn row(&self, record: u32) -> TestResult<&'catalog Value> {
        let rows = self
            .catalog
            .pointer("/definitions/lootPoolFilters/rows")
            .and_then(Value::as_array)
            .ok_or("actual canonical Loot rows")?;
        assert_eq!(
            rows.len(),
            2,
            "the two actual physical Records yield exactly two canonical rows"
        );
        let mut found = rows
            .iter()
            .filter(|row| row.pointer("/recordIndex/value") == Some(&json!(record)));
        let row = found.next().ok_or("actual physical header owner")?;
        assert!(
            found.next().is_none(),
            "the fixture has exactly one canonical physical row per Record"
        );
        Ok(row)
    }

    pub(crate) fn exact(&self, cell: &Value, expected: &[ExpectedUse]) -> TestResult {
        self.physical(cell)?;
        origin::verify(
            cell,
            self.inventory,
            &ExpectedOrigin {
                encrypted: self.input.marker(),
                complete: true,
                uses: expected,
            },
        )
        .map_err(|problem| format!("exact original contributor set: {problem:?}").into())
    }

    pub(crate) fn id(&self, expected: &ExpectedUse) -> TestResult<&'catalog str> {
        let mut matched = self.inventory.uses().iter().filter(|observed| {
            observed.role() == expected.role
                && observed.location() == &expected.location
                && observed.scope() == InputScope::Payload
        });
        let actual = matched
            .next()
            .ok_or("genuinely emitted original source-use ID")?;
        assert!(
            matched.next().is_none(),
            "this fixture has one actual source for each role/coordinate"
        );
        Ok(actual.id().as_str())
    }

    pub(crate) fn rejected(&self, cell: &Value, expected: &[ExpectedUse]) {
        assert_eq!(
            origin::verify(
                cell,
                self.inventory,
                &ExpectedOrigin {
                    encrypted: self.input.marker(),
                    complete: true,
                    uses: expected,
                }
            ),
            Err(SourceSetProblem::Contributions),
            "only the exact role/coordinate/cardinality mismatch rejects this genuine-ID mutation"
        );
    }

    fn physical(&self, cell: &Value) -> TestResult {
        for reference in cell
            .get("sourceUsesRefs")
            .and_then(Value::as_array)
            .ok_or("closed cell references")?
        {
            let id = reference.as_str().ok_or("source-use ID")?;
            let observed = self
                .inventory
                .uses()
                .iter()
                .find(|used| used.id().as_str() == id)
                .ok_or("same-result source-use closure")?;
            let source = self
                .inventory
                .sources()
                .iter()
                .find(|source| source.id() == observed.source())
                .ok_or("same-result physical-source closure")?;
            let source = serde_json::to_value(source)?;
            let (kind, path, expected) = if matches!(self.input, Input::Standalone) {
                ("file", "/name/value", "external.dcb")
            } else {
                ("archiveEntry", "/path/value", "Data\\Game2.dcb")
            };
            assert_eq!(
                source.get("kind"),
                Some(&json!(kind)),
                "the actual physical input kind stays distinct"
            );
            assert_eq!(
                source.pointer(path),
                Some(&json!(expected)),
                "no locale/foreign source supplies a DCB datum"
            );
        }
        Ok(())
    }
}

pub(crate) fn header(record: u32, field: &str, role: SourceUseRole) -> ExpectedUse {
    ExpectedUse {
        role,
        location: SourceLocation::Subresource {
            namespace: "dataforgeRecordHeader".to_owned(),
            indices: vec![u64::from(record)],
            field: field.to_owned(),
        },
    }
}

pub(crate) fn traversal(field: &str, role: SourceUseRole) -> ExpectedUse {
    ExpectedUse {
        role,
        location: SourceLocation::Subresource {
            namespace: "dataforgeTraversal".to_owned(),
            indices: vec![0, 0],
            field: field.to_owned(),
        },
    }
}

pub(crate) fn with_output(
    input: Input,
    inspect: impl FnOnce(&Output<'_>) -> TestResult,
) -> TestResult {
    let source = fixture::open(input)?;
    loot_reference_v3_source::check(&source.entry, input)?;
    assert_eq!(
        source.entry.origin().matches_archive(&source.archive),
        !matches!(input, Input::Standalone),
        "actual bound Archive versus standalone input"
    );
    let built = p4kx_export::build_catalog_from(
        &source.archive,
        &source.entry,
        &CatalogOptions::new(UNIX_EPOCH),
    )?;
    assert_eq!(
        built.summary().export_health().status,
        ExportHealthStatus::Complete,
        "all original inputs and references are readable"
    );
    assert_eq!(
        built
            .summary()
            .kind(Kind::LootPoolFilters)
            .map(|kind| kind.count),
        Some(2),
        "both actual physical source Records yield canonical rows"
    );
    let catalog: Value = serde_json::from_slice(built.json())?;
    let output = Output {
        catalog: &catalog,
        inventory: built.summary().source_inventory(),
        input,
    };
    output.count(0, 11)?;
    output.count(1, 10)?;
    inspect(&output)
}
