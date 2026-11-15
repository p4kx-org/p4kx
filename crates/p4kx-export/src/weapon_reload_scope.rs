// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use p4kx_extraction::{CancelToken, SourceLocation, SourceUses, SourceValue, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::catalog_read::original::PhysicalEntryRef;
use crate::schema_fields::every_field_required;

use super::assets::{AssetLink, Assets, FileSelector};
use super::document::check_cancel;
use super::graph::attribute;

#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct Declaration {
    pub(super) source: PhysicalEntryRef,
    pub(super) position: SourcedValue<SourceLocation>,
    pub(super) name: SourcedValue<String>,
    pub(super) sub_tag_definition: SourcedValue<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct Scope {
    pub(super) selector: AssetLink,
    pub(super) declarations: Vec<Declaration>,
    imports: Vec<ScopeImport>,
    coverage: SourcedValue<bool>,
    #[serde(skip)]
    #[schemars(skip)]
    names: BTreeMap<String, Vec<usize>>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct ScopeImport {
    source: PhysicalEntryRef,
    position: SourcedValue<SourceLocation>,
    target: AssetLink,
}

impl Scope {
    pub(super) fn read(
        selector: &SourcedValue<String>,
        assets: &mut Assets<'_, '_, '_>,
        sources: &SourceUses,
        cancel: Option<&CancelToken>,
    ) -> Result<Self, CatalogError> {
        let selector = assets.select(selector, FileSelector::Direct)?;
        let mut pending = VecDeque::new();
        let mut complete = if let Some(root) = selector.source.as_ref()
            && assets.definitions.contains_key(root)
        {
            pending.push_back(root.clone());
            true
        } else {
            false
        };
        let coverage =
            sources.value_with_origins(SourceValue::Present(complete), &[selector.origin()])?;
        let mut scope = Self {
            selector,
            declarations: Vec::new(),
            imports: Vec::new(),
            coverage,
            names: BTreeMap::new(),
        };
        let mut visited = BTreeSet::new();
        while let Some(next) = pending.pop_front() {
            check_cancel(cancel)?;
            if visited.insert(next.clone()) {
                scope.read_definition(&next, assets, sources, &mut pending, &mut complete)?;
            }
        }
        let mut origins = vec![scope.selector.origin()];
        origins.extend(visited.iter().filter_map(|entry| {
            assets
                .definitions
                .get(entry)
                .map(|def| def.version.origin())
        }));
        origins.extend(scope.imports.iter().map(|import| import.target.origin()));
        scope.coverage = sources.value_with_origins(SourceValue::Present(complete), &origins)?;
        Ok(scope)
    }

    pub(super) const fn coverage(&self) -> &SourcedValue<bool> {
        &self.coverage
    }

    pub(super) fn named(&self, spelling: &str) -> impl Iterator<Item = &Declaration> {
        self.names
            .get(&spelling.to_ascii_lowercase())
            .into_iter()
            .flatten()
            .filter_map(|position| self.declarations.get(*position))
    }

    fn read_definition(
        &mut self,
        entry: &PhysicalEntryRef,
        assets: &mut Assets<'_, '_, '_>,
        sources: &SourceUses,
        pending: &mut VecDeque<PhysicalEntryRef>,
        complete: &mut bool,
    ) -> Result<(), CatalogError> {
        let Some(definition) = assets.definitions.get(entry) else {
            *complete = false;
            return Ok(());
        };
        *complete &= definition.readable_version();
        for tag in &definition.tags {
            let name = attribute(&tag.attributes, "name", &tag.position, sources)?;
            let sub_tag_definition =
                attribute(&tag.attributes, "subTagDef", &tag.position, sources)?;
            let position = self.declarations.len();
            if let Some(label) = name
                .value()
                .filter(|label| !label.is_empty() && label.is_ascii())
            {
                self.names
                    .entry(label.to_ascii_lowercase())
                    .or_default()
                    .push(position);
            } else {
                *complete = false;
            }
            self.declarations.push(Declaration {
                source: entry.clone(),
                position: tag.position.clone(),
                name,
                sub_tag_definition,
            });
        }
        let selectors = definition
            .imports
            .iter()
            .map(|node| {
                Ok((
                    node.position.clone(),
                    attribute(&node.attributes, "filename", &node.position, sources)?,
                ))
            })
            .collect::<Result<Vec<_>, CatalogError>>()?;
        for (position, file) in selectors {
            let target = assets.select(&file, FileSelector::Direct)?;
            if let Some(target) = target.source.as_ref()
                && assets.definitions.contains_key(target)
            {
                pending.push_back(target.clone());
            } else {
                *complete = false;
            }
            self.imports.push(ScopeImport {
                source: entry.clone(),
                position,
                target,
            });
        }
        Ok(())
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct DatabaseScopes {
    pub(super) database: PhysicalEntryRef,
    pub(super) tags: Scope,
    pub(super) fragments: Scope,
}

pub(super) fn databases(
    assets: &mut Assets<'_, '_, '_>,
    sources: &SourceUses,
    cancel: Option<&CancelToken>,
) -> Result<Vec<DatabaseScopes>, CatalogError> {
    let selectors = assets
        .databases
        .iter()
        .map(|(id, database)| {
            Ok((
                id.clone(),
                attribute(&database.attributes, "TagDef", &database.position, sources)?,
                attribute(&database.attributes, "FragDef", &database.position, sources)?,
            ))
        })
        .collect::<Result<Vec<_>, CatalogError>>()?;
    selectors
        .into_iter()
        .map(|(database, tags, fragments)| {
            check_cancel(cancel)?;
            Ok(DatabaseScopes {
                database,
                tags: Scope::read(&tags, assets, sources, cancel)?,
                fragments: Scope::read(&fragments, assets, sources, cancel)?,
            })
        })
        .collect()
}
