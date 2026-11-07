// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::Kind;
use crate::contract_missions::ContractEntry;
use crate::export_diagnostic::{
    ExportDiagnostic, ExportDiagnosticCode, ExportDiagnosticExample, ExportDiagnosticSeverity,
    ExportDiagnosticStage, ExportDiagnosticUnit, ExportSourceRole,
};
use crate::export_ledger::{DerivedFailure, ExportAccountingError, ExportLedger, SourceDiagnosis};
use crate::mission_property_set::{MissionPropertySets, SetOwner};

const CODE: &str = "ambiguous_mission_property_set";

/// Removes ambiguous joins and records each affected physical Generator owner.
pub(super) fn finalize(
    sets: &MissionPropertySets<'_>,
    contracts: &mut [ContractEntry<'_>],
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
        failure(owner, ledger);
    }
    Ok(())
}

fn failure(owner: &SetOwner<'_>, ledger: &mut ExportLedger) {
    let diagnosis = SourceDiagnosis {
        code: Some(ExportDiagnosticCode::AmbiguousIdentity),
        field: Some(owner.location.clone()),
        source_type: Some(owner.source_type.to_owned()),
        target_guid: None,
    };
    for (affected_kind, role) in [
        (Kind::Contracts, ExportSourceRole::Selected),
        (Kind::MissionPropertySets, ExportSourceRole::DerivedInput),
    ] {
        ledger.derived_failure(DerivedFailure {
            affected_kind,
            output_kind: Kind::MissionPropertySets,
            record_index: owner.record_index,
            role,
            guid: owner.guid.0,
            record_path: Some(owner.path.to_owned()),
            diagnosis: diagnosis.clone(),
        });
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
        examples: vec![ExportDiagnosticExample {
            record_index: Some(owner.record_index),
            record_guid: Some(owner.guid.0.to_string()),
            record_path: Some(owner.path.to_owned()),
            field: Some(owner.location.clone()),
            source_type: Some(owner.source_type.to_owned()),
            target_guid: None,
        }],
    });
}
