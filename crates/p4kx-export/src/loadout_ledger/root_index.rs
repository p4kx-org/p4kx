// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Record, SourceUseRole, SourceValue, SourcedValue};

use super::Capture;
use crate::catalog_read::CatalogRecordContext;
use crate::contract_loadout::EntityKind;
use crate::entity_selection::{self, Selected};
use crate::guid_text::CatalogGuid;
use crate::loadout_owner_value::LoadoutOwnerValue;
use crate::record_header_field::{RecordHeaderField, RecordHeaderInput};
use crate::{CatalogError, ExportAccountingError, Kind};

pub(in crate::loadout) struct RootValues {
    pub(in crate::loadout) index: SourcedValue<u32>,
    pub(in crate::loadout) guid: LoadoutOwnerValue<CatalogGuid>,
    pub(in crate::loadout) kind: LoadoutOwnerValue<EntityKind>,
}

impl Capture<'_> {
    pub(in crate::loadout) fn root_values(
        &self,
        record: &Record<'_>,
        kind: EntityKind,
    ) -> Result<RootValues, CatalogError> {
        let guid = CatalogGuid(record.guid());
        #[cfg(test)]
        if self.input.is_none() {
            return Ok(RootValues {
                index: crate::contract_loadout::fixture_record_index(record.index()),
                guid: LoadoutOwnerValue::fixture(guid),
                kind: LoadoutOwnerValue::fixture(kind),
            });
        }
        let input = self
            .input
            .ok_or(ExportAccountingError::UnboundHealthSubject)?;
        let context = input.record_context(record)?;
        let kind = owner_kind(record, kind, context)?;
        Ok(RootValues {
            index: context.header_selector(
                RecordHeaderField::RecordIndex,
                SourceValue::Present(record.index()),
            )?,
            guid: LoadoutOwnerValue::new(
                guid,
                context.header_value(RecordHeaderField::Guid, SourceValue::Present(()))?,
            ),
            kind,
        })
    }
}

fn owner_kind(
    record: &Record<'_>,
    kind: EntityKind,
    context: CatalogRecordContext<'_>,
) -> Result<LoadoutOwnerValue<EntityKind>, CatalogError> {
    let selected = entity_selection::select(record, None).map_err(CatalogError::DataForge)?;
    let selection = match kind {
        EntityKind::Ship => Kind::Ships,
        EntityKind::Item => Kind::Items,
    };
    if selected != Some(Selected::Entity(selection)) {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    }
    let mut uses = vec![
        context.observe_header(RecordHeaderField::SourceType, SourceUseRole::Selector)?,
        context.observe_header(RecordHeaderField::RecordName, SourceUseRole::Selector)?,
    ];
    context.append_header_input(
        RecordHeaderInput::NormalizedPath,
        SourceUseRole::Selector,
        &mut uses,
    )?;
    let proof = context
        .sources()
        .value(SourceValue::Present(()), &uses, true);
    Ok(LoadoutOwnerValue::new(kind, proof))
}
