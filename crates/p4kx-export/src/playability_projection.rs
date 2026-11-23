// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    FieldOrigin, Guid, Record, RecordTree, SourceLocation, SourceUseRole, SourceValue,
    SourcedValue, StructValue, Value,
};

use super::playability_rules::{self, Evidence};
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_read::original::{OriginalFlowFields, OriginalValueFact, OriginalValuePart};
use crate::record_header_field::{RecordHeaderField, RecordHeaderInput};
use crate::selection::observe_exosuit_control;
use crate::{CatalogError, Playability, VehicleKind};

pub(crate) enum OwnerProof {
    Exosuit(SourcedValue<()>),
    Vehicle {
        predicates: FieldOrigin,
        links: FieldOrigin,
        targets: Vec<Guid>,
    },
}

impl OwnerProof {
    pub(super) fn capture(
        original: &OriginalCatalogInput<'_, '_>,
        record: &Record<'_>,
        fields: &OriginalFlowFields<'_, '_, '_>,
        vehicle: Option<&Value<'_>>,
        crew: Option<&Value<'_>>,
    ) -> Result<Self, CatalogError> {
        if let Some(exosuit) = observe_exosuit_control(original, record, fields)? {
            return Ok(Self::Exosuit(exosuit));
        }
        let mut predicates = Cells::new(fields);
        let context = original.record_context(record)?;
        context.append_header_input(
            RecordHeaderInput::NormalizedPath,
            SourceUseRole::Selector,
            &mut predicates.uses,
        )?;
        context.append_header_input(
            RecordHeaderInput::Field(RecordHeaderField::MetadataTag),
            SourceUseRole::Selector,
            &mut predicates.uses,
        )?;
        predicates.inclusion()?;
        if playability_rules::spacecraft(record.record_path().map_err(CatalogError::DataForge)?) {
            predicates.hull(vehicle)?;
        }

        let mut links = Cells::new(fields);
        let mut targets = Vec::new();
        links.array_prefix("Components", crew)?;
        if let Some(owner) = crew {
            let owner = fields.fact(owner)?;
            if let Some((_, link)) = fields.field(Some(owner), "crewManifest")? {
                links.reference(link, &mut targets, true)?;
            } else {
                links.uses.push(fields.survey(Some(owner))?);
            }
        }
        if let Some((_, tags)) = fields.field(None, "tags")? {
            match tags.value() {
                Value::Array(slots) => {
                    links.observe(tags, SourceUseRole::Selector)?;
                    for slot in slots {
                        links.reference(fields.fact(slot)?, &mut targets, false)?;
                    }
                }
                _ => links.value(tags, |value| matches!(value, Value::Null))?,
            }
        } else {
            links.uses.push(fields.survey(None)?);
        }
        targets.sort_unstable();
        targets.dedup();
        Ok(Self::Vehicle {
            predicates: predicates.finish(),
            links: links.finish(),
            targets,
        })
    }

    pub(crate) fn exosuit_kind(&self) -> Option<SourcedValue<VehicleKind>> {
        match self {
            Self::Exosuit(proof) => Some(proof.map_ref(|()| VehicleKind::Exosuit)),
            Self::Vehicle { .. } => None,
        }
    }

    pub(crate) fn decide(
        &self,
        tree: &RecordTree<'_>,
        vehicle: Option<&StructValue<'_>>,
        evidence: &Evidence,
        input: &OriginalCatalogInput<'_, '_>,
    ) -> Result<SourcedValue<Playability>, CatalogError> {
        let (predicates, links, targets) = match self {
            Self::Exosuit(proof) => return Ok(proof.map_ref(|()| Playability::Player)),
            Self::Vehicle {
                predicates,
                links,
                targets,
            } => (predicates, links, targets),
        };
        let (decision, unreadable_dependency) =
            playability_rules::ship_decision(tree, vehicle, evidence);
        let dependencies = evidence.provenance(input, targets)?;
        let mut origins = vec![links, &dependencies];
        if !unreadable_dependency {
            origins.push(predicates);
        }
        let state = match decision {
            Some(value) => SourceValue::Present(value),
            None if unreadable_dependency
                || origins.iter().any(|origin| !origin.source_set_complete()) =>
            {
                SourceValue::Unreadable
            }
            None => SourceValue::Absent,
        };
        Ok(input.origins().uses.value_with_origins(state, &origins)?)
    }
}

struct Cells<'fields, 'scope, 'read, 'source> {
    fields: &'fields OriginalFlowFields<'scope, 'read, 'source>,
    uses: Vec<p4kx_extraction::SourceUse>,
    complete: bool,
}

impl<'fields, 'scope, 'read, 'source> Cells<'fields, 'scope, 'read, 'source> {
    const fn new(fields: &'fields OriginalFlowFields<'scope, 'read, 'source>) -> Self {
        Self {
            fields,
            uses: Vec::new(),
            complete: true,
        }
    }

    fn finish(self) -> FieldOrigin {
        self.fields.sources().origin(&self.uses, self.complete)
    }

    fn observe(
        &mut self,
        fact: &OriginalValueFact<'scope, 'read, 'source>,
        role: SourceUseRole,
    ) -> Result<(), CatalogError> {
        self.uses
            .push(fact.observe(OriginalValuePart::Value, role)?);
        Ok(())
    }

    fn value(
        &mut self,
        fact: &OriginalValueFact<'scope, 'read, 'source>,
        recognized: impl FnOnce(&Value<'source>) -> bool,
    ) -> Result<(), CatalogError> {
        if recognized(fact.value()) {
            self.observe(fact, SourceUseRole::Selector)
        } else {
            self.complete = false;
            self.observe(fact, SourceUseRole::Attempted)
        }
    }

    fn leaf(
        &mut self,
        owner: Option<&OriginalValueFact<'scope, 'read, 'source>>,
        name: &str,
        recognized: impl FnOnce(&Value<'source>) -> bool,
    ) -> Result<(), CatalogError> {
        if let Some((_, fact)) = self.fields.field(owner, name)? {
            self.value(fact, recognized)
        } else {
            self.uses.push(self.fields.survey(owner)?);
            Ok(())
        }
    }

    fn array_prefix(
        &mut self,
        name: &str,
        selected: Option<&Value<'_>>,
    ) -> Result<(), CatalogError> {
        let prefix = self.fields.root_array_selection(name, selected)?;
        self.uses.extend(prefix.uses);
        self.complete &= prefix.complete;
        Ok(())
    }

    fn inclusion(&mut self) -> Result<(), CatalogError> {
        let Some((_, field)) = self.fields.field(None, "StaticEntityClassData")? else {
            return self.array_prefix("StaticEntityClassData", None);
        };
        let Value::Array(values) = field.value() else {
            return self.array_prefix("StaticEntityClassData", None);
        };
        let selected = values
            .iter()
            .find(|value| playability_rules::inclusion_text(value).is_some());
        self.array_prefix("StaticEntityClassData", selected)?;
        for value in values {
            if let Value::Struct(owner) = value
                && owner.type_name() == "EAEntityDataParams"
            {
                let owner_fact = self.fields.fact(value)?;
                self.leaf(Some(owner_fact), "inclusionMode", |value| {
                    matches!(value, Value::Null | Value::String(_) | Value::Enum(_))
                })?;
            }
            if selected.is_some_and(|selected| std::ptr::eq(selected, value)) {
                break;
            }
        }
        Ok(())
    }

    fn hull(&mut self, vehicle: Option<&Value<'_>>) -> Result<(), CatalogError> {
        self.array_prefix("Components", vehicle)?;
        let Some(vehicle) = vehicle else {
            return Ok(());
        };
        let owner = self.fields.fact(vehicle)?;
        self.leaf(Some(owner), "criticalPartExplosionChance", |value| {
            matches!(value, Value::Null) || crate::record_fields::number_value(value).is_some()
        })?;
        let Some((_, options)) = self.fields.field(Some(owner), "aiEngineeringOptions")? else {
            self.uses.push(self.fields.survey(Some(owner))?);
            return Ok(());
        };
        self.value(options, |value| {
            matches!(value, Value::Struct(_) | Value::Null)
        })?;
        if matches!(options.value(), Value::Struct(_)) {
            self.leaf(Some(options), "powerPlantsCanExplodeOnDeath", |value| {
                matches!(value, Value::Bool(_) | Value::Null)
            })?;
        }
        Ok(())
    }

    fn reference(
        &mut self,
        fact: &OriginalValueFact<'scope, 'read, 'source>,
        targets: &mut Vec<Guid>,
        root_only: bool,
    ) -> Result<(), CatalogError> {
        if let Value::Reference(reference) = fact.value()
            && (!root_only || reference.location().is_none())
        {
            let original = fact.reference_observation()?.guid_selector()?;
            let (uses, complete) = original.contributions();
            self.uses.extend_from_slice(uses);
            self.complete &= complete;
            targets.push(reference.guid());
            return Ok(());
        }
        if let Value::Error(error) = fact.value()
            && let Some(guid) = error.dangling_guid()
        {
            let original = fact
                .dangling_guid(SourceUseRole::Selector)?
                .ok_or(crate::ExportAccountingError::ConflictingSourceDatum)?;
            let (uses, complete) = original.contributions();
            self.uses.extend_from_slice(uses);
            self.complete &= complete;
            targets.push(guid);
            return Ok(());
        }
        self.value(fact, |value| matches!(value, Value::Null))
    }
}

pub(super) fn attempted_body(index: u32) -> SourceLocation {
    SourceLocation::Subresource {
        namespace: "dataforgeRecordBody".to_owned(),
        indices: vec![u64::from(index)],
        field: "body".to_owned(),
    }
}
