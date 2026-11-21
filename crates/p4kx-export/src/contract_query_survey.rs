// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};

use p4kx_extraction::{CancelToken, Guid, RecordTree, StructValue, Value};

use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_read::CatalogRecordContext;
use crate::catalog_read::original::OriginalFlowFields;
use crate::mission_generator::entries as mission_generator_entries;
use crate::mission_intros::{AuthoredIntro, Grants, Requirements};
use crate::mission_property_set_registry::links::MissionPropertySetLinks;
use crate::record_fields::identity;
use crate::record_observation::RecordObservation;
use crate::record_read::ReadError;
use crate::template_field_capture::TemplateFieldCapture;

pub(super) type LogicalKey = (Guid, Option<Guid>);

/// Source-only candidates used to select whole-Generator validation dependencies.
pub(super) struct Survey {
    pub(super) keys: BTreeSet<LogicalKey>,
    pub(super) rows: BTreeMap<LogicalKey, RowDependencies>,
    pub(super) property_sets: BTreeSet<Guid>,
    pub(super) grants: Grants,
}

#[derive(Default)]
pub(super) struct RowDependencies {
    required_tags: BTreeSet<Guid>,
    property_sets: BTreeSet<Guid>,
}

struct SourceRow<'tree, 'source> {
    key: LogicalKey,
    handler: &'tree StructValue<'source>,
    contract: &'tree StructValue<'source>,
    sub: Option<&'tree StructValue<'source>>,
}

impl Survey {
    pub(super) fn new(input: &OriginalCatalogInput<'_, '_>) -> Self {
        Self {
            keys: BTreeSet::new(),
            rows: BTreeMap::new(),
            property_sets: BTreeSet::new(),
            grants: Grants::new(input.origins()),
        }
    }

    pub(super) fn read<'source>(
        tree: &RecordTree<'source>,
        input: &OriginalCatalogInput<'_, 'source>,
        cancel: Option<&CancelToken>,
    ) -> Result<Self, ReadError> {
        let mut survey = Self::new(input);
        survey.read_tree(tree, input, None, cancel)?;
        Ok(survey)
    }

    pub(super) fn read_tree<'source>(
        &mut self,
        tree: &RecordTree<'source>,
        input: &OriginalCatalogInput<'_, 'source>,
        target: Option<LogicalKey>,
        cancel: Option<&CancelToken>,
    ) -> Result<(), ReadError> {
        let record = input
            .forge()
            .record(tree.record_index())
            .ok_or("original owner")?;
        let scope = input.record_scope(&record, tree)?;
        let fields = OriginalFlowFields::read(&scope, cancel)?;
        let context = input.record_context(&record)?;
        let rows = source_rows(tree, &fields, &context, &mut self.grants, &mut self.keys)?;
        self.property_sets = property_sets(tree, &fields)?;
        let admission = RecordObservation::capture(input, &record)?;
        let capture = TemplateFieldCapture::new(&fields, input, &admission)?;
        let links = target
            .map(|_| MissionPropertySetLinks::new(tree))
            .transpose()?;
        for row in rows {
            if target.is_some_and(|target| target != row.key) {
                continue;
            }
            let required =
                Requirements::shared(&fields, row.handler, row.contract)?.row(&fields, row.sub)?;
            let dependencies = self.rows.entry(row.key).or_default();
            if let Some(required) = required.value() {
                dependencies
                    .required_tags
                    .extend(required.iter().filter_map(AuthoredIntro::tag_guid));
            }
            if let Some(links) = &links {
                dependencies.property_sets.extend(
                    links
                        .row(row.handler, row.contract, row.sub, &capture)?
                        .into_iter()
                        .map(|guid| guid.0),
                );
            }
        }
        Ok(())
    }

    pub(super) fn relates_to(&self, wanted: &Wanted) -> bool {
        !self.keys.is_disjoint(&wanted.keys)
            || !self.property_sets.is_disjoint(&wanted.property_sets)
            || wanted
                .required_tags
                .iter()
                .any(|tag| self.grants.contains_tag(*tag))
    }
}

#[derive(Default)]
pub(super) struct Wanted {
    pub(super) keys: BTreeSet<LogicalKey>,
    pub(super) required_tags: BTreeSet<Guid>,
    pub(super) property_sets: BTreeSet<Guid>,
}

impl Wanted {
    pub(super) fn extend(&mut self, survey: &Survey) {
        self.keys.extend(survey.keys.iter().copied());
        for row in survey.rows.values() {
            self.required_tags.extend(row.required_tags.iter().copied());
        }
        self.property_sets
            .extend(survey.property_sets.iter().copied());
    }

    pub(super) fn extend_key(&mut self, survey: &Survey, key: LogicalKey) {
        if let Some(row) = survey.rows.get(&key) {
            self.required_tags.extend(row.required_tags.iter().copied());
            self.property_sets.extend(row.property_sets.iter().copied());
        }
    }
}

fn property_sets(
    tree: &RecordTree<'_>,
    fields: &OriginalFlowFields<'_, '_, '_>,
) -> Result<BTreeSet<Guid>, CatalogError> {
    let mut sets = BTreeSet::new();
    for fact in fields.facts() {
        fields.check_cancel()?;
        if fact.field().name() == "propertyOverrides" && !matches!(fact.value(), Value::Null) {
            let location = fields.logical_path(fact)?;
            sets.insert(
                crate::mission_property_set_registry::identity(tree.guid(), &location)
                    .map_err(CatalogError::Json)?
                    .0,
            );
        }
    }
    Ok(sets)
}

fn source_rows<'tree, 'source>(
    tree: &'tree RecordTree<'source>,
    fields: &OriginalFlowFields<'_, '_, 'source>,
    context: &CatalogRecordContext<'_>,
    grants: &mut Grants,
    keys: &mut BTreeSet<LogicalKey>,
) -> Result<Vec<SourceRow<'tree, 'source>>, ReadError> {
    let mut rows = Vec::new();
    mission_generator_entries::handlers(tree, fields, context, grants, |handler, grants| {
        mission_generator_entries::candidates(tree, handler, fields, grants, |candidate| {
            mission_generator_entries::rows(tree, candidate.contract, |sub| {
                let key = (
                    identity(candidate.contract).ok_or("admitted contract identity")?,
                    sub.and_then(identity),
                );
                keys.insert(key);
                rows.push(SourceRow {
                    key,
                    handler,
                    contract: candidate.contract,
                    sub,
                });
                Ok(())
            })
        })
    })?;
    Ok(rows)
}
