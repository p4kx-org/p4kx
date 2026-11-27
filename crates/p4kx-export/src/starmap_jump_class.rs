// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use p4kx_extraction::{
    CancelToken, DataForge, Guid, Record, RecordTree, SourceUse, SourceUseRole, SourceValue,
    TreeBuilder, Value,
};

use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_read::original::{OriginalFlowFields, OriginalValuePart};
use crate::record_header_field::RecordHeaderField;
use crate::{CatalogError, CatalogOptions, Kind};

#[derive(Debug, Default)]
pub(crate) struct JumpEndpointRecords {
    records: HashMap<Guid, EndpointClass>,
    pub(super) complete: bool,
}

#[derive(Debug)]
pub(crate) struct EndpointClass {
    pub(crate) index: u32,
    pub(crate) index_input: SourceUse,
    pub(crate) name: &'static str,
    pub(crate) inputs: Vec<SourceUse>,
}

impl JumpEndpointRecords {
    pub(crate) fn read<'source>(
        forge: &'source DataForge<'source>,
        input: &OriginalCatalogInput<'_, 'source>,
        options: &CatalogOptions<'_>,
    ) -> Result<Self, CatalogError> {
        input.check_forge(forge)?;
        let mut endpoints = Self {
            records: HashMap::new(),
            complete: true,
        };
        let mut tree_builder = TreeBuilder::new();
        let mut types = HashMap::new();
        let rule = Kind::StarmapObjects.records();
        for record in forge.records() {
            if options.cancel.is_some_and(CancelToken::is_cancelled) {
                return Err(CatalogError::Cancelled);
            }
            let Ok(path) = record.record_path() else {
                endpoints.complete = false;
                continue;
            };
            if !rule.matches_path(path)
                || options.filter.is_some_and(|filter| !filter.matches(path))
            {
                continue;
            }
            let Ok(source_type) = record.type_name() else {
                endpoints.complete = false;
                continue;
            };
            if source_type != rule.type_name {
                continue;
            }
            let Ok(tree) = tree_builder.build(&record) else {
                endpoints.complete = false;
                continue;
            };
            let Some(Value::Reference(reference)) = tree.field("type") else {
                endpoints.complete &= !matches!(tree.field("type"), Some(Value::Error(_)));
                continue;
            };
            let key = reference.guid();
            let class =
                match types.entry(key) {
                    std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
                    std::collections::hash_map::Entry::Vacant(entry) => entry.insert(
                        endpoint_class(forge, input, key, &mut tree_builder, options.cancel)?,
                    ),
                };
            endpoints.remember_record(forge, input, &record, &tree, class, options.cancel)?;
        }
        Ok(endpoints)
    }

    fn remember_record(
        &mut self,
        forge: &DataForge<'_>,
        input: &OriginalCatalogInput<'_, '_>,
        record: &Record<'_>,
        tree: &RecordTree<'_>,
        class: &SourceValue<(&'static str, SourceUse)>,
        cancel: Option<&CancelToken>,
    ) -> Result<(), CatalogError> {
        match class {
            SourceValue::Present((name, type_source)) => {
                if forge.records_by_guid(record.guid()).nth(1).is_some() {
                    self.complete = false;
                    return Ok(());
                }
                self.records.insert(
                    record.guid(),
                    EndpointClass::read(input, record, tree, (name, type_source), cancel)?,
                );
            }
            SourceValue::Invalid | SourceValue::Unreadable => self.complete = false,
            _ => {}
        }
        Ok(())
    }

    pub(crate) const fn is_complete(&self) -> bool {
        self.complete
    }

    pub(crate) fn get(&self, guid: Guid) -> Option<&EndpointClass> {
        self.records.get(&guid)
    }
}

impl EndpointClass {
    fn read(
        input: &OriginalCatalogInput<'_, '_>,
        record: &Record<'_>,
        tree: &RecordTree<'_>,
        (name, type_source): (&'static str, &SourceUse),
        cancel: Option<&CancelToken>,
    ) -> Result<Self, CatalogError> {
        let scope = input.record_scope(record, tree)?;
        let originals = OriginalFlowFields::read(&scope, cancel)?;
        let (_, fact) = originals
            .field(None, "type")?
            .ok_or(crate::ExportAccountingError::ConflictingSourceDatum)?;
        let type_ref = fact.observe(OriginalValuePart::Value, SourceUseRole::Selector)?;
        let index_input = input
            .record_context(record)?
            .observe_header(RecordHeaderField::RecordIndex, SourceUseRole::Selector)?;
        Ok(Self {
            index: record.index(),
            index_input,
            name,
            inputs: vec![type_ref, type_source.clone()],
        })
    }
}

fn endpoint_class<'source>(
    forge: &'source DataForge<'source>,
    input: &OriginalCatalogInput<'_, 'source>,
    guid: Guid,
    trees: &mut TreeBuilder<'source>,
    cancel: Option<&CancelToken>,
) -> Result<SourceValue<(&'static str, SourceUse)>, CatalogError> {
    let mut candidates = forge.records_by_guid(guid);
    let Some(record) = candidates.next() else {
        return Ok(SourceValue::Invalid);
    };
    if candidates.next().is_some() || record.type_name().ok() != Some("StarMapObjectType") {
        return Ok(SourceValue::Invalid);
    }
    let Ok(tree) = trees.build(&record) else {
        return Ok(SourceValue::Unreadable);
    };
    let name = match tree.field("name") {
        Some(Value::String("JumpPoint")) => "JumpPoint",
        Some(Value::String("Anomaly")) => "Anomaly",
        Some(Value::Error(_)) => return Ok(SourceValue::Unreadable),
        _ => return Ok(SourceValue::Absent),
    };
    let scope = input.record_scope(&record, &tree)?;
    let originals = OriginalFlowFields::read(&scope, cancel)?;
    let (_, fact) = originals
        .field(None, "name")?
        .ok_or(crate::ExportAccountingError::ConflictingSourceDatum)?;
    Ok(SourceValue::Present((
        name,
        fact.observe(OriginalValuePart::Value, SourceUseRole::Selector)?,
    )))
}
