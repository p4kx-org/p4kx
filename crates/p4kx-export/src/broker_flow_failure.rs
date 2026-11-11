// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::export_diagnostic::ExportSourceRole;
use crate::export_ledger::{DerivedFailure, ExportLedger};
use crate::record_observation::RecordObservation;
use crate::{CatalogError, CatalogReadFailure, Kind};

/// A failed required Broker projection paired with its successfully emitted raw Flow owner.
pub(crate) struct BrokerProjectionFailure<'source> {
    pub(crate) ledger: ExportLedger,
    pub(crate) failure: CatalogReadFailure<'source>,
}

impl<'source> BrokerProjectionFailure<'source> {
    pub(crate) fn read(
        admission: &RecordObservation<'_, 'source>,
        error: &(dyn std::error::Error + 'static),
    ) -> Result<Self, CatalogError> {
        let diagnosis = crate::export_source_failure::diagnosis(error);
        let mut ledger = ExportLedger::default();
        for kind in [
            Kind::MissionBrokerEntries,
            Kind::ContractShipSpawns,
            Kind::ContractNpcSpawns,
            Kind::ContractEntitySpawns,
            Kind::ContractLocationQueries,
        ] {
            ledger.derived_failure(DerivedFailure::of_subject(
                admission.subject().clone(),
                kind,
                Kind::MissionBrokerEntries,
                ExportSourceRole::DerivedInput,
                diagnosis.clone(),
            )?);
        }
        Ok(Self {
            ledger,
            failure: CatalogReadFailure::of(admission.record(), error),
        })
    }
}
