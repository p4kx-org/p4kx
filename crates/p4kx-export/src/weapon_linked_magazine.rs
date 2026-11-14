// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;

use p4kx_extraction::{
    CancelToken, DataForge, DataForgeError, FieldOrigin, Record, RecordTree, SourceEvidenceError,
    SourceUseRole, Value,
};

use super::linked_selection::LinkedSlots;
use super::route;
use super::{ContainerAmmoSlots, OriginalFlowFields, ProjectileAmmoRole};
use crate::catalog_projection::CatalogProjection;
use crate::catalog_read::original::OriginalValueFact;
use crate::catalog_v3::authored_fields::Observed;
use crate::catalog_v3::catalog_item_components::resolution::SelectionEvidence;
use crate::catalog_v3::catalog_weapon_magazine_slots::MAGAZINE_TARGET;
use crate::catalog_v3::item_component_owner::ComponentOwner;
use crate::export_diagnostic::ExportSourceRole;
use crate::export_ledger::{ExportLedger, SourceDisposition};
use crate::item_stats::{MagazineRoute, PRIMARY_MAGAZINE_FIELD};
use crate::record_header_field::RecordHeaderField;
use crate::record_observation::RecordObservation;
use crate::source_value_catalog::SourceRecordReference;
use crate::template_field_capture::TemplateFieldCapture;
use crate::{CatalogError, ExportAccountingError, Kind};

#[derive(Clone, Copy)]
pub(super) struct LinkedMagazineRead<'read, 'source> {
    caller: &'read Value<'source>,
    record: Record<'source>,
    tree: Result<&'read RecordTree<'source>, &'read DataForgeError>,
}

impl<'read, 'source> LinkedMagazineRead<'read, 'source> {
    pub(super) const fn capture(route: MagazineRoute<'read, 'source>) -> Option<Self> {
        match route {
            MagazineRoute::Own(_) => None,
            MagazineRoute::Linked {
                caller,
                record,
                tree,
                ..
            } => Some(Self {
                caller,
                record,
                tree,
            }),
        }
    }
}

pub(crate) struct LinkedAmmunitionReader<'context, 'source> {
    pub(crate) projection: &'context CatalogProjection<'context, 'source>,
    pub(crate) forge: &'source DataForge<'source>,
    pub(crate) cancel: Option<&'context CancelToken>,
}

pub(super) enum LinkedMagazine<'source> {
    Unique(Box<LinkedSlots<'source>>),
    SelfReference,
    AmbiguousReference(FieldOrigin),
    Unreadable(FieldOrigin),
    NoContainer(FieldOrigin),
    AmbiguousContainer(FieldOrigin),
    IncompleteComponents(FieldOrigin),
}

impl<'source> LinkedMagazine<'source> {
    pub(super) fn read(
        reader: &LinkedAmmunitionReader<'_, 'source>,
        fields: &OriginalFlowFields<'_, '_, 'source>,
        weapon: &ComponentOwner,
        linked: LinkedMagazineRead<'_, 'source>,
        reference: &SourceRecordReference<'source>,
        ledger: &mut ExportLedger,
    ) -> Result<Self, CatalogError> {
        let (component, _) =
            route::original_component(fields, route::components(fields)?, weapon.key())?;
        let original = fields.fact(component)?;
        let (_, caller) = fields
            .field(Some(original), PRIMARY_MAGAZINE_FIELD)?
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        caller.check_input(reader.projection.original())?;
        let observation = caller.reference_observation()?;
        if !std::ptr::eq(caller.value(), linked.caller)
            || observation.record_index() != linked.record.index()
            || reference.resolved_index() != Some(linked.record.index())
            || observation.is_instance()
            || !observation
                .reference()
                .original_target()
                .shares_source_with(&linked.record)
            || linked.record.type_name().map_err(CatalogError::DataForge)? != MAGAZINE_TARGET
        {
            return Err(SourceEvidenceError::ForeignEvidence.into());
        }
        if linked.record.index() == fields.record_index() {
            return Ok(Self::SelfReference);
        }
        let mut candidates = reader.forge.records_by_guid(linked.record.guid());
        let candidate = candidates
            .next()
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        if candidate.index() != linked.record.index() {
            return Err(SourceEvidenceError::ForeignEvidence.into());
        }
        if let Some(second) = candidates.next() {
            let input = reader.projection.original();
            let mut evidence = SelectionEvidence::default();
            for candidate in [candidate, second].into_iter().chain(candidates) {
                fields.check_cancel()?;
                let context = input.record_context(&candidate)?;
                evidence
                    .add(context.observe_header(RecordHeaderField::Guid, SourceUseRole::Selector)?);
                evidence.add(
                    context
                        .observe_header(RecordHeaderField::RecordIndex, SourceUseRole::Selector)?,
                );
            }
            return Ok(Self::AmbiguousReference(
                evidence.into_origin(fields.sources()),
            ));
        }
        reader.read_admitted(linked, ledger)
    }
}

impl<'source> LinkedAmmunitionReader<'_, 'source> {
    fn read_admitted(
        &self,
        linked: LinkedMagazineRead<'_, 'source>,
        ledger: &mut ExportLedger,
    ) -> Result<LinkedMagazine<'source>, CatalogError> {
        let record = linked.record;
        let input = self.projection.original();
        let admission = RecordObservation::capture(input, &record)?;
        let context = input.record_context(&record)?.with_cancel(self.cancel);
        let tree = match linked.tree {
            Err(DataForgeError::ObservationStopped) => return Err(CatalogError::Cancelled),
            Err(error) => {
                let mut evidence = SelectionEvidence::default();
                evidence.attempt(context.observe("", SourceUseRole::Attempted)?);
                ledger.observe(
                    admission
                        .source(
                            Some(Kind::Items),
                            ExportSourceRole::Dependency,
                            SourceDisposition::ReadFailed,
                        )?
                        .diagnosed(crate::export_source_failure::diagnosis(error)),
                );
                return Ok(LinkedMagazine::Unreadable(
                    evidence.into_origin(context.sources()),
                ));
            }
            Ok(tree) => tree,
        };
        ledger.observe(admission.source(
            Some(Kind::Items),
            ExportSourceRole::Dependency,
            SourceDisposition::Projected,
        )?);
        ledger.merge(crate::export_field_walk::inspect_observed(
            Some(input),
            tree,
            &admission,
            Some(Kind::Items),
            ExportSourceRole::Dependency,
            self.cancel,
        )?);
        let scope = input.record_scope(&record, tree)?;
        let fields = OriginalFlowFields::read(&scope, self.cancel)?;
        let inventory = route::linked_container(&fields, input)?;
        match inventory {
            route::LinkedInventory::Unique {
                container,
                evidence,
            } => self.read_container(&fields, tree, &admission, container, evidence, ledger),
            route::LinkedInventory::NoContainer(evidence) => {
                Ok(LinkedMagazine::NoContainer(evidence))
            }
            route::LinkedInventory::Competing(evidence) => {
                Ok(LinkedMagazine::AmbiguousContainer(evidence))
            }
            route::LinkedInventory::Incomplete(evidence) => {
                Ok(LinkedMagazine::IncompleteComponents(evidence))
            }
        }
    }

    fn read_container<'scope, 'read>(
        &self,
        fields: &OriginalFlowFields<'scope, 'read, 'source>,
        tree: &RecordTree<'source>,
        admission: &RecordObservation<'_, 'source>,
        container: &OriginalValueFact<'scope, 'read, 'source>,
        survey: FieldOrigin,
        ledger: &mut ExportLedger,
    ) -> Result<LinkedMagazine<'source>, CatalogError> {
        let input = self.projection.original();
        let context = input
            .record_context(admission.record())?
            .with_cancel(self.cancel);
        let capture = TemplateFieldCapture::new(fields, input, admission)?;
        let owner = ComponentOwner::read(tree, &context, fields, container)?;
        let numbers = RefCell::new(Vec::new());
        let observed = Observed::new(fields, input, container, &capture, &numbers);
        let mut notes = Vec::new();
        let slots = ContainerAmmoSlots::read(owner, observed, &mut notes)?;
        let Value::Struct(body) = container.value() else {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        };
        let linked = LinkedSlots {
            slots,
            evidence: survey,
            declarations: fields
                .sources()
                .origin(&[fields.survey(Some(container))?], true),
            unique_primary: route::at_most_one_declaration(
                body,
                ProjectileAmmoRole::Primary.field_name(),
            ),
            unique_secondary: route::at_most_one_declaration(
                body,
                ProjectileAmmoRole::Secondary.field_name(),
            ),
        };
        crate::source_projection_notes::record(
            Kind::Items,
            ExportSourceRole::Dependency,
            admission,
            tree,
            notes,
            ledger,
        )?;
        Ok(LinkedMagazine::Unique(Box::new(linked)))
    }
}
