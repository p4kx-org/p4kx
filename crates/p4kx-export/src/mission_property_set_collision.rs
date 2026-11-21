// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Guid;

use crate::Kind;
use crate::contract_missions::ContractBase;
use crate::export_diagnostic::{
    ExportDiagnosticCode, ExportDiagnosticSeverity, ExportDiagnosticStage, ExportDiagnosticUnit,
    ExportSourceRole,
};
use crate::export_ledger::{DerivedFailure, ExportAccountingError, ExportLedger, SourceDiagnosis};
use crate::health_diagnostic_observation::{
    DiagnosticObservation as ExportDiagnostic, ExampleObservation,
};
use crate::mission_property_set::{MissionPropertySets, SetOwner};
use crate::record_derived_observation::DerivedOperation;

const CODE: &str = "ambiguous_mission_property_set";

/// Removes ambiguous joins and records each affected physical Generator owner.
pub(super) fn finalize<'read, 'source: 'read>(
    sets: &MissionPropertySets<'_>,
    contracts: impl IntoIterator<Item = &'read mut ContractBase<'source>>,
    ledger: &mut ExportLedger,
) -> Result<(), ExportAccountingError> {
    for (_, owner) in sets.ambiguous_owners() {
        if !ledger.sources.iter().any(|source| {
            source.record_index == owner.record_index
                && source.kind == Some(Kind::MissionPropertySets)
                && matches!(
                    source.role,
                    ExportSourceRole::DerivedInput | ExportSourceRole::Dependency
                )
                && source.disposition == crate::export_ledger::SourceDisposition::Projected
        }) {
            return Err(ExportAccountingError::UnobservedFailure);
        }
    }
    for contract in contracts {
        contract
            .property_set_guids
            .retain(|guid| !sets.is_ambiguous(*guid));
    }
    for (guid, owner) in sets.ambiguous_owners() {
        failure(guid, owner, ledger)?;
    }
    Ok(())
}

fn failure(
    guid: Guid,
    owner: &SetOwner<'_>,
    ledger: &mut ExportLedger,
) -> Result<(), ExportAccountingError> {
    let body = owner.body()?;
    let diagnosis = SourceDiagnosis {
        code: Some(ExportDiagnosticCode::AmbiguousIdentity),
        field: Some(owner.location.clone()),
        source_type: Some(owner.source_type.to_owned()),
        target_guid: None,
        body: crate::record_source_status::DiagnosisBody::Captured(body.clone()),
    };
    let contract_role = source_role(ledger, owner.record_index, Kind::Contracts)?;
    let property_role = source_role(ledger, owner.record_index, Kind::MissionPropertySets)?;
    for (affected_kind, role) in [
        (Kind::Contracts, contract_role),
        (Kind::MissionPropertySets, property_role),
    ] {
        ledger.derived_failure(DerivedFailure::of_operation(
            owner.subject().clone(),
            affected_kind,
            Kind::MissionPropertySets,
            role,
            DerivedOperation::MissionPropertySet(guid),
            diagnosis.clone(),
        )?);
    }
    ledger.field_diagnostic(ExportDiagnostic {
        kind: Some(Kind::MissionPropertySets),
        stage: ExportDiagnosticStage::DerivedProjection,
        code: ExportDiagnosticCode::AmbiguousIdentity,
        severity: ExportDiagnosticSeverity::Error,
        source_role: Some(property_role),
        warning_code: Some(CODE.to_owned()),
        count: 1,
        count_unit: ExportDiagnosticUnit::Values,
        examples: vec![ExampleObservation::captured(
            owner.subject().clone(),
            Some(&owner.location),
            Some(owner.source_type),
            None,
            body.clone(),
        )?],
    });
    Ok(())
}

fn source_role(
    ledger: &ExportLedger,
    record: u32,
    kind: Kind,
) -> Result<ExportSourceRole, ExportAccountingError> {
    ledger
        .sources
        .iter()
        .find(|source| {
            source.record_index == record
                && source.kind == Some(kind)
                && source.disposition == crate::export_ledger::SourceDisposition::Projected
        })
        .map(|source| source.role)
        .ok_or(ExportAccountingError::UnobservedFailure)
}
