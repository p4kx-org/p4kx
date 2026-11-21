// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use crate::Kind;
use crate::build::original_input::test_support::TestResult;
use crate::export_diagnostic::ExportSourceRole;
use crate::export_ledger::{ExportAccountingError, SourceDisposition};

use super::TargetIndex;

#[path = "catalog_broker_row_source_fixture.rs"]
mod fixture;

#[test]
fn an_actual_emitted_broker_owner_requires_its_selected_projected_canonical_source() -> TestResult {
    fixture::with_actual(|actual| {
        let indexed = TargetIndex::new(
            std::slice::from_ref(&actual.row),
            &[],
            std::slice::from_ref(&actual.source),
            &actual.canonical,
        )?;
        assert!(
            indexed
                .emitted
                .contains(&(Kind::MissionBrokerEntries, actual.source.record_index)),
            "the actual D owner is bound to its genuine selected projected physical source"
        );
        assert!(
            matches!(
                TargetIndex::new(
                    std::slice::from_ref(&actual.row),
                    &[],
                    &[],
                    &actual.canonical
                ),
                Err(ExportAccountingError::MissingCatalogRowSource {
                    kind: Kind::MissionBrokerEntries
                })
            ),
            "an output row alone never proves an original source"
        );
        for (role, disposition) in [
            (ExportSourceRole::Dependency, SourceDisposition::Projected),
            (
                ExportSourceRole::Selected,
                SourceDisposition::ProjectionFailed,
            ),
            (ExportSourceRole::Selected, SourceDisposition::ReadFailed),
        ] {
            let mut rejected = actual.source.clone();
            rejected.role = role;
            rejected.disposition = disposition;
            assert!(
                matches!(
                    TargetIndex::new(
                        std::slice::from_ref(&actual.row),
                        &[],
                        &[rejected],
                        &actual.canonical
                    ),
                    Err(ExportAccountingError::MissingCatalogRowSource {
                        kind: Kind::MissionBrokerEntries
                    })
                ),
                "the real original receipt cannot waive role/read/projection admission"
            );
        }
        Ok(())
    })
}

#[test]
fn broker_d_identity_and_canonical_inspection_guards_remain_mandatory() -> TestResult {
    fixture::with_actual(|actual| {
        assert!(
            matches!(
                TargetIndex::new(
                    std::slice::from_ref(&actual.row),
                    &[],
                    std::slice::from_ref(&actual.source),
                    &BTreeMap::new()
                ),
                Err(ExportAccountingError::MissingFieldInspection)
            ),
            "the actual row/source pair still requires the canonical original inspection"
        );
        let mut wrong_row = actual.row.clone();
        wrong_row.record_path.push_str(".wrong");
        assert!(
            matches!(
                TargetIndex::new(
                    &[wrong_row],
                    &[],
                    std::slice::from_ref(&actual.source),
                    &actual.canonical
                ),
                Err(ExportAccountingError::MissingCatalogRowSource {
                    kind: Kind::MissionBrokerEntries
                })
            ),
            "a copied row descriptor with a different path is not an emitted original"
        );
        let mut wrong_source = actual.source.clone();
        wrong_source.guid = "00000000-0000-0000-0000-000000000000".parse()?;
        assert!(
            matches!(
                TargetIndex::new(
                    std::slice::from_ref(&actual.row),
                    &[],
                    &[wrong_source],
                    &actual.canonical
                ),
                Err(ExportAccountingError::ConflictingFieldInspection)
            ),
            "the genuine inspection must agree with the original source GUID"
        );
        let mut wrong_family = actual.source;
        wrong_family.kind = Some(Kind::ContractTemplates);
        assert!(
            matches!(
                TargetIndex::new(&[actual.row], &[], &[wrong_family], &actual.canonical),
                Err(ExportAccountingError::MissingCatalogRowSource {
                    kind: Kind::MissionBrokerEntries
                })
            ),
            "another selected family cannot authorize this physical Broker D row"
        );
        Ok(())
    })
}
