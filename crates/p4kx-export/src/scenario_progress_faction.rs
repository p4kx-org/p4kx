// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;

use p4kx_extraction::{
    CancelToken, Record, SourceUseRole, SourceValue, SourceValueState, SourcedValue, TreeBuilder,
};

use super::super::super::authored_fields::Observed;
use super::ReferenceCell;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_query_session::checkpoint;
use crate::catalog_read::original::OriginalFlowFields;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::definition_reference::DefinitionStatus;
use crate::export_diagnostic::ExportSourceRole;
use crate::export_field_walk;
use crate::export_ledger::{ExportLedger, SourceDisposition};
use crate::record_observation::RecordObservation;
use crate::source_value_catalog::SourceRecordReference;
use crate::template_field_capture::TemplateFieldCapture;
use crate::{CatalogError, ExportAccountingError, Kind};

const FACTION: &str = "Faction";
const REPUTATION: &str = "factionReputationRef";
const ROLE: ExportSourceRole = ExportSourceRole::Dependency;

pub(super) struct FactionFields<'source> {
    pub(super) name: SourcedValue<&'source str>,
    pub(super) reputation: SourcedValue<CatalogRecordReference>,
}

/// Observes only the selected physical Faction's original reputation member.
pub(super) struct FactionReader<'read, 'source> {
    input: &'read OriginalCatalogInput<'read, 'source>,
    cancel: Option<&'read CancelToken>,
    ledger: ExportLedger,
    trees: TreeBuilder<'read>,
}

impl<'read, 'source> FactionReader<'read, 'source> {
    pub(super) fn new(
        input: &'read OriginalCatalogInput<'read, 'source>,
        cancel: Option<&'read CancelToken>,
    ) -> Self {
        Self {
            input,
            cancel,
            ledger: ExportLedger::default(),
            trees: TreeBuilder::new(),
        }
    }

    pub(super) fn finish(self) -> ExportLedger {
        self.ledger
    }

    pub(super) fn read(
        &mut self,
        faction: &ReferenceCell<'source>,
    ) -> Result<FactionFields<'source>, CatalogError> {
        checkpoint(self.cancel)?;
        let input = self.input;
        let sources = &input.origins().uses;
        let Some(reference) = faction.value() else {
            let operands = &[faction.origin()];
            return Ok(FactionFields {
                name: sources.value_with_origins(unchosen(faction.state())?, operands)?,
                reputation: sources.value_with_origins(unchosen(faction.state())?, operands)?,
            });
        };
        let Some(position) = reference
            .resolved_index()
            .filter(|_| reference.target_type().value().copied() == Some(FACTION))
        else {
            let absent = matches!(reference.status(), DefinitionStatus::Dangling);
            let operands = &[faction.origin(), reference.target_type().origin()];
            return Ok(FactionFields {
                name: sources.value_with_origins(rejected(absent), operands)?,
                reputation: sources.value_with_origins(rejected(absent), operands)?,
            });
        };
        let record = input
            .forge()
            .record(position)
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        let name = reference
            .record_path()
            .map_ref(|&path| path.rsplit('/').next().unwrap_or(path));
        let name = sources.add_value_origins(name, &[faction.origin()])?;
        let reputation = self.reputation(&record)?;
        Ok(FactionFields {
            name,
            reputation: sources.add_value_origins(reputation, &[faction.origin()])?,
        })
    }

    fn reputation(
        &mut self,
        record: &Record<'read>,
    ) -> Result<SourcedValue<CatalogRecordReference>, CatalogError> {
        let admission = RecordObservation::capture(self.input, record)?;
        let tree = match self.trees.build(record) {
            Ok(tree) => tree,
            Err(error) => {
                let source = admission.source(
                    Some(Kind::ContractScenarios),
                    ROLE,
                    SourceDisposition::ReadFailed,
                )?;
                self.ledger
                    .observe(source.diagnosed(crate::export_source_failure::diagnosis(&error)));
                let origins = self.input.origins();
                let attempt = origins.dataforge.at(
                    crate::playability_projection::attempted_body(record.index()),
                    SourceUseRole::Attempted,
                )?;
                return Ok(origins
                    .uses
                    .value(SourceValue::Unreadable, &[attempt], false));
            }
        };
        self.ledger.merge(export_field_walk::inspect_record_field(
            self.input,
            &tree,
            &admission,
            Kind::ContractScenarios,
            REPUTATION,
            self.cancel,
        )?);
        self.ledger.observe(admission.source(
            Some(Kind::ContractScenarios),
            ROLE,
            SourceDisposition::Projected,
        )?);
        let scope = self.input.record_scope(record, &tree)?;
        let originals = OriginalFlowFields::read(&scope, self.cancel)?;
        let capture = TemplateFieldCapture::new(&originals, self.input, &admission)?;
        let numbers = RefCell::new(Vec::new());
        let observed = Observed::definition(&originals, self.input, &capture, &numbers);
        let mut notes = Vec::new();
        let reputation = observed.record_link(REPUTATION, &mut notes)?;
        crate::source_projection_notes::record(
            Kind::ContractScenarios,
            ROLE,
            &admission,
            &tree,
            notes,
            &mut self.ledger,
        )?;
        Ok(reputation.map(|reference| SourceRecordReference::into_catalog(*reference)))
    }
}

const fn rejected<T>(absent: bool) -> SourceValue<T> {
    if absent {
        SourceValue::Absent
    } else {
        SourceValue::Invalid
    }
}

fn unchosen<T>(state: SourceValueState) -> Result<SourceValue<T>, CatalogError> {
    Ok(match state {
        SourceValueState::Absent => SourceValue::Absent,
        SourceValueState::Null => SourceValue::Null,
        SourceValueState::Unreadable => SourceValue::Unreadable,
        SourceValueState::Unsupported => SourceValue::Unsupported,
        SourceValueState::Invalid => SourceValue::Invalid,
        _ => return Err(ExportAccountingError::ConflictingSourceDatum.into()),
    })
}
