// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

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

const CODE: &str = "ambiguous_mission_property_set";

/// Removes ambiguous joins and records each affected physical Generator owner.
pub(super) fn finalize<'read, 'source: 'read>(
    sets: &MissionPropertySets<'_>,
    contracts: impl IntoIterator<Item = &'read mut ContractBase<'source>>,
    ledger: &mut ExportLedger,
) -> Result<(), ExportAccountingError> {
    for owner in sets.ambiguous_owners() {
        if !ledger.sources.iter().any(|source| {
            source.record_index == owner.record_index
                && source.kind == Some(Kind::MissionPropertySets)
                && source.role == ExportSourceRole::DerivedInput
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
    for owner in sets.ambiguous_owners() {
        failure(owner, ledger)?;
    }
    Ok(())
}

fn failure(owner: &SetOwner<'_>, ledger: &mut ExportLedger) -> Result<(), ExportAccountingError> {
    let body = owner.body()?;
    let diagnosis = SourceDiagnosis {
        code: Some(ExportDiagnosticCode::AmbiguousIdentity),
        field: Some(owner.location.clone()),
        source_type: Some(owner.source_type.to_owned()),
        target_guid: None,
        body: crate::record_source_status::DiagnosisBody::Captured(body.clone()),
    };
    for (affected_kind, role) in [
        (Kind::Contracts, ExportSourceRole::Selected),
        (Kind::MissionPropertySets, ExportSourceRole::DerivedInput),
    ] {
        ledger.derived_failure(DerivedFailure::of_subject(
            owner.subject().clone(),
            affected_kind,
            Kind::MissionPropertySets,
            role,
            diagnosis.clone(),
        )?);
    }
    ledger.field_diagnostic(ExportDiagnostic {
        kind: Some(Kind::MissionPropertySets),
        stage: ExportDiagnosticStage::DerivedProjection,
        code: ExportDiagnosticCode::AmbiguousIdentity,
        severity: ExportDiagnosticSeverity::Error,
        source_role: Some(ExportSourceRole::DerivedInput),
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
