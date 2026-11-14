// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Guid;

use crate::build::original_input::test_support::{self, TestResult};
use crate::catalog_read::original::OriginalFlowFields;
use crate::export_diagnostic::ExportSourceRole;
use crate::export_ledger::{SourceDisposition, SourceObservation};
use crate::mission_flow_owner::{FlowOwnerKind, MissionFlowOwnerRef};
use crate::record_observation::RecordObservation;
use crate::{CatalogError, ExportAccountingError, Kind};

use super::super::selection::{self, Action};
use super::{NumberRequirement, TemplateNumbers, fixture, with_family};

#[test]
fn actual_broker_emission_covers_only_its_original_occurrences_and_admitted_roles() -> TestResult {
    test_support::with_input(&fixture::broker_bytes(), |forge, input| {
        let record = forge.record(0).ok_or("actual original Broker")?;
        let tree = record.tree()?;
        super::controls(&record, &tree)?;
        assert_eq!(
            record.type_name()?,
            "MissionBrokerEntry",
            "actual original owner family"
        );
        assert!(
            matches!(selection::choose(&record)?, Some(Action::Broker)),
            "the actual mission dependency selector chooses this Broker"
        );
        let scope = input.record_scope(&record, &tree)?;
        let fields = OriginalFlowFields::read(&scope, None)?;
        assert_owner(&fields, record.guid())?;
        let (numbers, requirements) =
            super::emission_kind(&fields, input, &record, Kind::MissionBrokerEntries)?;
        assert_eq!(
            numbers.emitted_kind(),
            Some(Kind::MissionBrokerEntries),
            "the physical emission family"
        );
        let admission = RecordObservation::capture(input, &record)?;
        let source = admission.source(
            Some(Kind::MissionBrokerEntries),
            ExportSourceRole::Selected,
            SourceDisposition::Projected,
        )?;
        coverage(&numbers, &requirements, &source)?;
        let dependency = admission.source(
            Some(Kind::Contracts),
            ExportSourceRole::Dependency,
            SourceDisposition::Projected,
        )?;
        coverage(&numbers, &requirements, &dependency)?;
        check_outside(&fields, &numbers, &requirements, &source)?;
        foreign_input(&numbers)
    })
}

fn assert_owner(fields: &OriginalFlowFields<'_, '_, '_>, guid: Guid) -> TestResult {
    let owner = MissionFlowOwnerRef::read(fields, FlowOwnerKind::MissionBrokerEntry)?;
    let wire = serde_json::to_value(owner)?;
    assert_eq!(
        wire.get("type"),
        Some(&serde_json::json!("missionBrokerEntry")),
        "the actual family tag"
    );
    assert_eq!(
        wire.get("recordGuid"),
        Some(&serde_json::json!(guid.to_string())),
        "actual original GUID"
    );
    assert!(
        matches!(
            MissionFlowOwnerRef::read(fields, FlowOwnerKind::ContractTemplate),
            Err(CatalogError::Accounting(
                ExportAccountingError::ConflictingSourceDatum
            ))
        ),
        "a real Broker frame cannot mint a Template owner by a family alias"
    );
    Ok(())
}

fn check_outside(
    fields: &OriginalFlowFields<'_, '_, '_>,
    numbers: &TemplateNumbers,
    requirements: &[NumberRequirement],
    source: &SourceObservation,
) -> TestResult {
    let mut outside = requirements
        .first()
        .ok_or("actual original numeric occurrence")?
        .clone();
    let fact = fields
        .field(None, "outside")?
        .ok_or("actual unselected equal number")?
        .1;
    outside.original_ordinal = fact.ordinal();
    outside.field = fields.logical_path(fact)?;
    outside.source_type = fact.owner_type().to_owned();
    with_family(
        numbers,
        Some(Kind::MissionBrokerEntries),
        source,
        |coverage| {
            assert!(
                !coverage.take(&outside),
                "the equal genuine outside occurrence remains blocking"
            );
            Ok(())
        },
    )
}

fn coverage(
    numbers: &TemplateNumbers,
    requirements: &[NumberRequirement],
    source: &SourceObservation,
) -> TestResult {
    assert_eq!(
        requirements.len(),
        4,
        "all four independent original NaN requirements"
    );
    with_family(
        numbers,
        Some(Kind::MissionBrokerEntries),
        source,
        |coverage| {
            for required in requirements {
                assert!(
                    coverage.take(required),
                    "the actual emitted same-original occurrence covers itself"
                );
            }
            assert!(
                !coverage.take(requirements.first().ok_or("actual first original number")?),
                "one original occurrence cannot cover a second requirement"
            );
            Ok(())
        },
    )?;
    for family in [None, Some(Kind::ContractTemplates)] {
        with_family(numbers, family, source, |coverage| {
            assert!(
                requirements.iter().all(|required| !coverage.take(required)),
                "missing or wrong-family D emission cannot cover this Broker"
            );
            Ok(())
        })?;
    }
    let mut wrong = source.clone();
    wrong.kind = Some(Kind::ContractTemplates);
    wrong.role = ExportSourceRole::Selected;
    with_family(
        numbers,
        Some(Kind::MissionBrokerEntries),
        &wrong,
        |coverage| {
            assert!(
                requirements.iter().all(|required| !coverage.take(required)),
                "the same actual subject cannot waive a different selected family"
            );
            Ok(())
        },
    )
}

fn foreign_input(numbers: &TemplateNumbers) -> TestResult {
    test_support::with_input(&fixture::broker_bytes(), |forge, input| {
        let record = forge.record(0).ok_or("actual separate Broker input")?;
        let admission = RecordObservation::capture(input, &record)?;
        let source = admission.source(
            Some(Kind::MissionBrokerEntries),
            ExportSourceRole::Selected,
            SourceDisposition::Projected,
        )?;
        assert!(
            numbers.for_source(&source)?.is_none(),
            "equal GUID/index/path/bits in a genuine different input never become the original emission"
        );
        Ok(())
    })
}
