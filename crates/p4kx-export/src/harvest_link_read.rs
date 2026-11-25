// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;
use std::sync::{Mutex, PoisonError};

use p4kx_extraction::{
    CancelToken, DataForge, FieldOrigin, Record, RecordTree, SourceLocation, SourceScope,
    SourceUseRole, SourceUses, TreeBuilder, Value,
};

use crate::CatalogError;
use crate::build::original_input::{
    OriginalCatalogInput, append_record_guid_inventory, record_body_location,
};
use crate::catalog_read::original::{OriginalFlowFields, OriginalValuePart};
use crate::export_field_error;
use crate::guid_text::CatalogGuid;
use crate::harvest_projection_fields::{Object, Projection};
use crate::record_fields::{class_name, record_reference};
use crate::record_header_field::RecordHeaderField;

use super::{MINEABLE_PARAMS, SelectedReference};

/// An original dependency lookup and the fields of its readable body.
#[derive(Clone)]
pub(super) struct Dependency<T> {
    pub(super) facts: Option<T>,
    pub(super) selection: FieldOrigin,
}

/// Tree and provenance authority for one Catalog or SDK read.
pub(crate) struct DependencyReader<'read, 'source> {
    pub(crate) trees: &'read mut TreeBuilder<'source>,
    pub(crate) forge: &'source DataForge<'source>,
    pub(crate) input: &'read OriginalCatalogInput<'read, 'source>,
    pub(crate) cancel: Option<&'read CancelToken>,
}

impl<'source> DependencyReader<'_, 'source> {
    pub(super) fn selected<T>(
        &mut self,
        guid: CatalogGuid,
        role: &str,
        unreadable: &Mutex<BTreeSet<String>>,
        project: impl FnOnce(&Self, &Record<'source>, &RecordTree<'source>) -> Result<T, CatalogError>,
    ) -> Result<Dependency<T>, CatalogError> {
        self.input.check_forge(self.forge)?;
        if self.cancel.is_some_and(CancelToken::is_cancelled) {
            return Err(CatalogError::Cancelled);
        }

        let mut uses = Vec::new();
        append_record_guid_inventory(self.input, &mut uses)?;
        let origins = self.input.origins();
        let (facts, complete, failure) = if let Some(record) = self.forge.record_by_guid(guid.0) {
            let header = self.input.record_context(&record)?;
            uses.push(header.observe_header(RecordHeaderField::Guid, SourceUseRole::Selector)?);
            if let Ok(tree) = self.trees.build(&record) {
                (Some(project(self, &record, &tree)?), true, None)
            } else {
                uses.push(origins.dataforge.at(
                    record_body_location(record.index()),
                    SourceUseRole::Attempted,
                )?);
                (None, false, Some("cannot be read"))
            }
        } else {
            (None, true, Some("is not in the DataForge"))
        };
        if let Some(failure) = failure {
            unreadable
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .insert(format!("{role} {} {failure}", guid.0));
        }
        Ok(Dependency {
            facts,
            selection: origins.uses.origin(&uses, complete),
        })
    }
}

#[derive(Clone)]
pub(super) struct PresetFacts<'source> {
    pub(super) class_name: Option<&'source str>,
    pub(super) class_origin: FieldOrigin,
    pub(super) entity: PresetReference,
}

pub(super) fn preset<'source>(
    reader: &DependencyReader<'_, 'source>,
    record: &Record<'source>,
    tree: &RecordTree<'source>,
) -> Result<PresetFacts<'source>, CatalogError> {
    let scope = reader.input.record_scope(record, tree)?;
    let original = OriginalFlowFields::read(&scope, reader.cancel)?;
    let header = reader.input.record_context(record)?;
    let class_origin = original.sources().origin(
        &[
            header.observe_header(RecordHeaderField::RecordName, SourceUseRole::Conversion)?,
            header.observe_header(RecordHeaderField::SourceType, SourceUseRole::Selector)?,
        ],
        true,
    );
    Ok(PresetFacts {
        class_name: Some(class_name(tree)),
        class_origin,
        entity: PresetReference::read(&original, header.dataforge())?,
    })
}

/// The preset fallback captured at its admitted Record and used only on that branch.
#[derive(Clone)]
pub(super) struct PresetReference {
    source: SourceScope,
    declaration: SourceLocation,
    value: Option<(SourceLocation, SourceUseRole)>,
    guid: Option<CatalogGuid>,
    complete: bool,
}

impl PresetReference {
    fn read(
        original: &OriginalFlowFields<'_, '_, '_>,
        source: &SourceScope,
    ) -> Result<Self, CatalogError> {
        let Some((_, fact)) = original.field(None, "entityClass")? else {
            return Ok(Self {
                source: source.clone(),
                declaration: original.survey_location(None)?,
                value: None,
                guid: None,
                complete: true,
            });
        };
        let value = fact.value();
        let guid = record_reference(value);
        let readable =
            !matches!(value, Value::Error(error) if export_field_error::blocks_projection(*error));
        let role = if guid.is_some() {
            SourceUseRole::Payload
        } else if readable {
            SourceUseRole::Selector
        } else {
            SourceUseRole::Attempted
        };
        Ok(Self {
            source: source.clone(),
            declaration: fact.location(OriginalValuePart::FieldName),
            value: Some((fact.location(OriginalValuePart::Value), role)),
            guid,
            complete: readable,
        })
    }

    pub(super) fn resolve(&self, sources: &SourceUses) -> Result<SelectedReference, CatalogError> {
        let mut uses = vec![
            self.source
                .at(self.declaration.clone(), SourceUseRole::Selector)?,
        ];
        if let Some((location, role)) = &self.value {
            uses.push(self.source.at(location.clone(), *role)?);
        }
        Ok(SelectedReference {
            guid: self.guid,
            origin: sources.origin(&uses, self.complete),
        })
    }
}

#[derive(Clone)]
pub(super) struct MiningFacts {
    pub(super) composition: SelectedReference,
    pub(super) global_params: SelectedReference,
}

pub(super) fn mining<'source>(
    reader: &DependencyReader<'_, 'source>,
    record: &Record<'source>,
    tree: &RecordTree<'source>,
) -> Result<MiningFacts, CatalogError> {
    let scope = reader.input.record_scope(record, tree)?;
    let original = OriginalFlowFields::read(&scope, reader.cancel)?;
    let projection = Projection::new(&original);
    let params = projection.first_component(MINEABLE_PARAMS)?;
    Ok(MiningFacts {
        composition: reference(&projection, &params, "composition")?,
        global_params: reference(&projection, &params, "globalParams")?,
    })
}

fn reference<'tree, 'source>(
    projection: &Projection<'_, '_, 'tree, 'source>,
    owner: &Object<'tree, 'source>,
    field: &str,
) -> Result<SelectedReference, CatalogError> {
    let selected = projection.value(owner, field, record_reference)?;
    Ok(SelectedReference {
        guid: selected.value,
        origin: projection.origin(&selected.inputs),
    })
}
