// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::Arc;

use p4kx_extraction::{CancelToken, SourceLocation, SourceUses, SourceValue, SourcedValue};
use schemars::JsonSchema;
use serde::ser::SerializeSeq;
use serde::{Serialize, Serializer};

use crate::CatalogError;
use crate::catalog_read::original::PhysicalEntryRef;
use crate::schema_fields::every_field_required;

use super::assets::{AssetLink, Assets, Definition, FileSelector};
use super::document::check_cancel;
use super::graph::attribute;

/// Original declaration reached in a tag or fragment scope.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct Declaration {
    /// Physical definition Entry that owns the declaration.
    pub source: PhysicalEntryRef,
    /// Original declaration coordinate.
    pub position: SourcedValue<SourceLocation>,
    /// Original authored name.
    pub name: SourcedValue<String>,
    /// Original nested tag-definition locator.
    pub sub_tag_definition: SourcedValue<String>,
}

/// A tag or fragment scope formed by an original database selector.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct Scope {
    pub(super) selector: AssetLink,
    #[serde(serialize_with = "serialize_declarations")]
    #[schemars(with = "Vec<Declaration>")]
    declarations: Vec<Arc<DeclaredTags>>,
    imports: Vec<ScopeImport>,
    coverage: SourcedValue<bool>,
}

/// Original import within a tag or fragment definition scope.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct ScopeImport {
    /// Original definition Entry containing the import.
    pub source: PhysicalEntryRef,
    /// Original import coordinate.
    pub position: SourcedValue<SourceLocation>,
    /// Authored target locator and resolution.
    pub target: AssetLink,
}

#[derive(Debug)]
struct DeclaredTags {
    rows: Vec<Declaration>,
    names: BTreeMap<String, Vec<usize>>,
    complete: bool,
}

impl DeclaredTags {
    fn read(
        entry: &PhysicalEntryRef,
        definition: &Definition,
        sources: &SourceUses,
    ) -> Result<Self, CatalogError> {
        let mut tags = Self {
            rows: Vec::with_capacity(definition.tags.len()),
            names: BTreeMap::new(),
            complete: true,
        };
        for tag in &definition.tags {
            let name = attribute(&tag.attributes, "name", &tag.position, sources)?;
            let sub_tag_definition =
                attribute(&tag.attributes, "subTagDef", &tag.position, sources)?;
            if let Some(label) = name
                .value()
                .filter(|label| !label.is_empty() && label.is_ascii())
            {
                tags.names
                    .entry(label.to_ascii_lowercase())
                    .or_default()
                    .push(tags.rows.len());
            } else {
                tags.complete = false;
            }
            tags.rows.push(Declaration {
                source: entry.clone(),
                position: tag.position.clone(),
                name,
                sub_tag_definition,
            });
        }
        Ok(tags)
    }
}

fn serialize_declarations<S: Serializer>(
    declarations: &[Arc<DeclaredTags>],
    serializer: S,
) -> Result<S::Ok, S::Error> {
    let count = declarations.iter().map(|tags| tags.rows.len()).sum();
    let mut sequence = serializer.serialize_seq(Some(count))?;
    for row in declaration_rows(declarations) {
        sequence.serialize_element(row)?;
    }
    sequence.end()
}

fn declaration_rows(declarations: &[Arc<DeclaredTags>]) -> impl Iterator<Item = &Declaration> {
    declarations.iter().flat_map(|tags| &tags.rows)
}

impl Scope {
    /// Borrows the original root selector.
    #[must_use]
    pub const fn selector(&self) -> &AssetLink {
        &self.selector
    }

    /// Iterates original declarations in the same order as the Catalog.
    pub fn declarations(&self) -> impl Iterator<Item = &Declaration> {
        declaration_rows(&self.declarations)
    }

    /// Borrows the followed import declarations in source order.
    #[must_use]
    pub fn imports(&self) -> &[ScopeImport] {
        &self.imports
    }

    fn read(
        selector: &SourcedValue<String>,
        assets: &mut Assets<'_, '_, '_>,
        sources: &SourceUses,
        cancel: Option<&CancelToken>,
        declarations: &mut BTreeMap<PhysicalEntryRef, Arc<DeclaredTags>>,
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
        };
        let mut visited = BTreeSet::new();
        while let Some(next) = pending.pop_front() {
            check_cancel(cancel)?;
            if visited.insert(next.clone()) {
                scope.read_definition(
                    &next,
                    assets,
                    sources,
                    &mut pending,
                    &mut complete,
                    declarations,
                )?;
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

    /// Borrows the observed scope-coverage state.
    #[must_use]
    pub const fn coverage(&self) -> &SourcedValue<bool> {
        &self.coverage
    }

    pub(super) fn named(&self, spelling: &str) -> impl Iterator<Item = &Declaration> {
        let spelling = spelling.to_ascii_lowercase();
        self.declarations.iter().flat_map(move |tags| {
            tags.names
                .get(&spelling)
                .into_iter()
                .flatten()
                .filter_map(move |position| tags.rows.get(*position))
        })
    }

    fn read_definition(
        &mut self,
        entry: &PhysicalEntryRef,
        assets: &mut Assets<'_, '_, '_>,
        sources: &SourceUses,
        pending: &mut VecDeque<PhysicalEntryRef>,
        complete: &mut bool,
        declarations: &mut BTreeMap<PhysicalEntryRef, Arc<DeclaredTags>>,
    ) -> Result<(), CatalogError> {
        let Some(definition) = assets.definitions.get(entry) else {
            *complete = false;
            return Ok(());
        };
        let tags = match declarations.entry(entry.clone()) {
            Entry::Occupied(existing) => Arc::clone(existing.get()),
            Entry::Vacant(vacant) => {
                let tags = DeclaredTags::read(entry, definition, sources)?;
                Arc::clone(vacant.insert(Arc::new(tags)))
            }
        };
        *complete &= definition.readable_version() && tags.complete;
        self.declarations.push(tags);
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

/// Tag and fragment definition scopes reached by one physical database.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct DatabaseScopes {
    /// Physical database that supplied both root selectors.
    pub database: PhysicalEntryRef,
    /// Authored tag scope and coverage.
    pub tags: Scope,
    /// Authored fragment scope and coverage.
    pub fragments: Scope,
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
    let mut declarations = BTreeMap::new();
    selectors
        .into_iter()
        .map(|(database, tags, fragments)| {
            check_cancel(cancel)?;
            Ok(DatabaseScopes {
                database,
                tags: Scope::read(&tags, assets, sources, cancel, &mut declarations)?,
                fragments: Scope::read(&fragments, assets, sources, cancel, &mut declarations)?,
            })
        })
        .collect()
}
