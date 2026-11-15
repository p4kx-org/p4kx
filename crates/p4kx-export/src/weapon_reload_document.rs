// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::cryxml::SourceElement;
use p4kx_extraction::cryxml::animation_database::{AnimationDatabase, FragmentOption, SubDatabase};
use p4kx_extraction::{CancelToken, SourceLocation, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::catalog_read::original::{EntrySourceXml, PhysicalEntryIdentity, SourceXmlAttribute};
use crate::schema_fields::every_field_required;

/// An original Player-reachable `AnimDB` and its physical child occurrences.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct Database {
    pub(super) source: PhysicalEntryIdentity,
    pub(super) position: SourcedValue<SourceLocation>,
    pub(super) attributes: Vec<SourceXmlAttribute>,
    pub(super) branches: Vec<Branch>,
    pub(super) options: Vec<OptionRow>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct Branch {
    pub(super) position: SourcedValue<SourceLocation>,
    ordinals: SourcedValue<[usize; 2]>,
    pub(super) attributes: Vec<SourceXmlAttribute>,
    pub(super) fragments: Vec<Node>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct OptionRow {
    pub(super) position: SourcedValue<SourceLocation>,
    ordinals: SourcedValue<[usize; 3]>,
    pub(super) fragment: SourcedValue<String>,
    pub(super) attributes: Vec<SourceXmlAttribute>,
    elements: Vec<Node>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct Node {
    pub(super) position: SourcedValue<SourceLocation>,
    ordinal: SourcedValue<usize>,
    parent: SourcedValue<Option<usize>>,
    name: SourcedValue<String>,
    pub(super) attributes: Vec<SourceXmlAttribute>,
}

impl Database {
    pub(super) fn read(
        source: &EntrySourceXml<'_>,
        cancel: Option<&CancelToken>,
    ) -> Result<Option<Self>, CatalogError> {
        let Some(database) = AnimationDatabase::from_root(source.root()) else {
            return Ok(None);
        };
        let branches = database
            .branches()
            .map(|branch| {
                check_cancel(cancel)?;
                Branch::read(source, branch)
            })
            .collect::<Result<_, _>>()?;
        let options = database
            .options()
            .map(|option| {
                check_cancel(cancel)?;
                OptionRow::read(source, option, cancel)
            })
            .collect::<Result<_, _>>()?;
        Ok(Some(Self {
            source: source.identity()?,
            position: source.position(database.root())?,
            attributes: source.attributes(database.root())?,
            branches,
            options,
        }))
    }
}

impl Branch {
    fn read(source: &EntrySourceXml<'_>, branch: SubDatabase<'_>) -> Result<Self, CatalogError> {
        let position = source.position(branch.element())?;
        let fragments = branch
            .fragments()
            .map(|(ordinal, element)| Node::read(source, element, ordinal, None))
            .collect::<Result<_, _>>()?;
        Ok(Self {
            ordinals: position.map_ref(|_| branch.ordinals().into()),
            position,
            attributes: source.attributes(branch.element())?,
            fragments,
        })
    }

    pub(super) const fn position(&self) -> &SourcedValue<SourceLocation> {
        &self.position
    }
}

impl OptionRow {
    fn read(
        source: &EntrySourceXml<'_>,
        option: FragmentOption<'_>,
        cancel: Option<&CancelToken>,
    ) -> Result<Self, CatalogError> {
        let position = source.position(option.element())?;
        Ok(Self {
            ordinals: position.map_ref(|_| option.ordinals().into()),
            position,
            fragment: source.name(option.group())?,
            attributes: source.attributes(option.element())?,
            elements: elements(source, option.element(), cancel)?,
        })
    }
}

impl Node {
    pub(super) const fn position(&self) -> &SourcedValue<SourceLocation> {
        &self.position
    }

    fn read(
        source: &EntrySourceXml<'_>,
        element: SourceElement<'_>,
        ordinal: usize,
        parent: Option<usize>,
    ) -> Result<Self, CatalogError> {
        let position = source.position(element)?;
        Ok(Self {
            ordinal: position.map_ref(|_| ordinal),
            parent: position.map_ref(|_| parent),
            position,
            name: source.name(element)?,
            attributes: source.attributes(element)?,
        })
    }
}

fn elements(
    source: &EntrySourceXml<'_>,
    parent: SourceElement<'_>,
    cancel: Option<&CancelToken>,
) -> Result<Vec<Node>, CatalogError> {
    let mut rows = Vec::new();
    let mut stack = vec![(None, parent.elements().enumerate())];
    while let Some((parent, children)) = stack.last_mut() {
        check_cancel(cancel)?;
        let Some((ordinal, element)) = children.next() else {
            stack.pop();
            continue;
        };
        let index = rows.len();
        rows.push(Node::read(source, element, ordinal, *parent)?);
        stack.push((Some(index), element.elements().enumerate()));
    }
    Ok(rows)
}

pub(super) fn check_cancel(cancel: Option<&CancelToken>) -> Result<(), CatalogError> {
    if cancel.is_some_and(CancelToken::is_cancelled) {
        Err(CatalogError::Cancelled)
    } else {
        Ok(())
    }
}
