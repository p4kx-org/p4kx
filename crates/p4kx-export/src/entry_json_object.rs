// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;

use p4kx_extraction::{
    DecimalU64, Guid, SourceUseRole, SourceUses, SourceValue, SourceValueState, SourcedValue,
};

use super::catalog_json_number::CatalogJsonNumber;
use crate::CatalogError;
use crate::catalog_read::original::EntryJson;
use crate::catalog_read::parsed_entry_json::{
    self, ParsedEntryJson, SourceMember, SourceNode, SourceNodeValue,
};
use crate::guid_text::CatalogGuid;

/// Original members of a JSON object, with no duplicate-key collapse.
pub(crate) struct EntryJsonObject<'object, 'read> {
    parsed: &'object ParsedEntryJson<'read>,
    root: &'object SourceNode<'read>,
    members: &'object [SourceMember<'read>],
    sources: &'object SourceUses,
    projected: Option<RefCell<Vec<usize>>>,
}

impl<'object, 'read> EntryJsonObject<'object, 'read> {
    pub(crate) fn new(
        parsed: &'object ParsedEntryJson<'read>,
        node: usize,
        sources: &'object SourceUses,
    ) -> Result<Option<Self>, CatalogError> {
        let root = parsed.node(node)?;
        let SourceNodeValue::Object(members) = &root.value else {
            return Ok(None);
        };
        Ok(Some(Self {
            parsed,
            root,
            members,
            sources,
            projected: None,
        }))
    }

    /// Tracks successfully represented members for an exact raw fallback.
    /// A malformed or repeated field remains available as its authored JSON tokens.
    pub(crate) fn retaining_unprojected(&self) -> Self {
        Self {
            parsed: self.parsed,
            root: self.root,
            members: self.members,
            sources: self.sources,
            projected: Some(RefCell::default()),
        }
    }

    pub(crate) fn field<T>(
        &self,
        name: &str,
        project: impl FnOnce(&SourceNode<'read>) -> SourceValue<T>,
    ) -> Result<SourcedValue<T>, CatalogError> {
        self.field_with(name, |node| Ok(project(node)), is_represented)
    }

    pub(crate) fn array<T>(
        &self,
        name: &str,
        mut project: impl FnMut(&'object SourceNode<'read>) -> SourceValue<T>,
    ) -> Result<SourcedValue<Vec<SourcedValue<T>>>, CatalogError> {
        self.array_with(name, |element| Ok(project(element)))
    }

    pub(crate) fn object_array<T>(
        &self,
        name: &str,
        mut project: impl FnMut(&Self) -> Result<T, CatalogError>,
    ) -> Result<SourcedValue<Vec<SourcedValue<T>>>, CatalogError> {
        self.array_with(name, |element| {
            self.object_node(element)
                .map_or(Ok(SourceValue::Invalid), |object| {
                    project(&object).map(SourceValue::Present)
                })
        })
    }

    fn object_node(&self, node: &'object SourceNode<'read>) -> Option<Self> {
        let SourceNodeValue::Object(members) = &node.value else {
            return None;
        };
        Some(Self {
            parsed: self.parsed,
            root: node,
            members,
            sources: self.sources,
            projected: self.projected.as_ref().map(|_| RefCell::default()),
        })
    }

    fn array_with<T>(
        &self,
        name: &str,
        mut project: impl FnMut(&'object SourceNode<'read>) -> Result<SourceValue<T>, CatalogError>,
    ) -> Result<SourcedValue<Vec<SourcedValue<T>>>, CatalogError> {
        self.field_with(
            name,
            |node| {
                let SourceNodeValue::Array(positions) = &node.value else {
                    return Ok(SourceValue::Invalid);
                };
                let mut members = Vec::new();
                members
                    .try_reserve_exact(positions.len())
                    .map_err(CatalogError::JsonSourceAllocation)?;
                for &position in positions {
                    let element = self.parsed.node(position)?;
                    let value = if matches!(element.value, SourceNodeValue::Null) {
                        SourceValue::Null
                    } else {
                        project(element)?
                    };
                    members.push(self.sources.value(
                        value,
                        std::slice::from_ref(&element.payload),
                        true,
                    ));
                }
                Ok(SourceValue::Present(members))
            },
            |value| match value {
                SourceValue::Null => true,
                SourceValue::Present(members) => members.iter().all(|member| {
                    matches!(
                        member.state(),
                        SourceValueState::Present | SourceValueState::Null
                    )
                }),
                _ => false,
            },
        )
    }

    /// Projects unrepresented original members in source order with their original tokens.
    pub(crate) fn additional_fields<T>(
        &self,
        mut project: impl FnMut(
            EntryJson<'read>,
            &SourceNode<'read>,
            &SourceUses,
        ) -> Result<T, CatalogError>,
    ) -> Result<Vec<T>, CatalogError> {
        let mut extra = Vec::new();
        let projected = self.projected.as_ref().map(RefCell::borrow);
        for member in self.members {
            if projected
                .as_ref()
                .is_some_and(|ids| ids.contains(&member.value))
            {
                continue;
            }
            let token = self.parsed.node(member.value)?;
            parsed_entry_json::push(&mut extra, project(member.name_token, token, self.sources)?)?;
        }
        Ok(extra)
    }

    fn field_with<T>(
        &self,
        name: &str,
        project: impl FnOnce(&'object SourceNode<'read>) -> Result<SourceValue<T>, CatalogError>,
        represented: impl FnOnce(&SourceValue<T>) -> bool,
    ) -> Result<SourcedValue<T>, CatalogError> {
        let mut matching = self
            .members
            .iter()
            .filter(|member| member.name.text() == Some(name));
        let Some(first) = matching.next() else {
            let selected = self.root.token.observe(SourceUseRole::Selector)?;
            return Ok(self.sources.value(SourceValue::Absent, &[selected], true));
        };
        let field = self.parsed.node(first.value)?;
        let own = parsed_entry_json::member_uses(first, field)?;
        let Some(second) = matching.next() else {
            let value = if matches!(field.value, SourceNodeValue::Null) {
                SourceValue::Null
            } else {
                project(field)?
            };
            if let Some(projected) = &self.projected
                && represented(&value)
            {
                let mut projected = projected.borrow_mut();
                if !projected.contains(&first.value) {
                    parsed_entry_json::push(&mut projected, first.value)?;
                }
            }
            return Ok(self.sources.value(value, &own, true));
        };
        let mut uses = Vec::from(own);
        for member in std::iter::once(second).chain(matching) {
            let node = self.parsed.node(member.value)?;
            for use_ in parsed_entry_json::member_uses(member, node)? {
                parsed_entry_json::push(&mut uses, use_)?;
            }
        }
        Ok(self.sources.value(SourceValue::Invalid, &uses, true))
    }
}

const fn is_represented<T>(value: &SourceValue<T>) -> bool {
    matches!(value, SourceValue::Present(_) | SourceValue::Null)
}

pub(crate) fn text(node: &SourceNode<'_>) -> SourceValue<String> {
    match &node.value {
        SourceNodeValue::String(value) => value.text().map_or(SourceValue::Invalid, |text| {
            SourceValue::Present(text.to_owned())
        }),
        _ => SourceValue::Invalid,
    }
}

pub(crate) const fn boolean(node: &SourceNode<'_>) -> SourceValue<bool> {
    match node.value {
        SourceNodeValue::Boolean(value) => SourceValue::Present(value),
        _ => SourceValue::Invalid,
    }
}

pub(crate) fn guid(node: &SourceNode<'_>) -> SourceValue<CatalogGuid> {
    match &node.value {
        SourceNodeValue::String(value) => value
            .text()
            .and_then(parse_guid)
            .map_or(SourceValue::Invalid, SourceValue::Present),
        _ => SourceValue::Invalid,
    }
}

fn parse_guid(raw: &str) -> Option<CatalogGuid> {
    let raw = raw
        .strip_prefix('{')
        .and_then(|inner| inner.strip_suffix('}'))
        .unwrap_or(raw);
    raw.parse::<Guid>().ok().map(CatalogGuid)
}

pub(crate) fn number(node: &SourceNode<'_>) -> SourceValue<CatalogJsonNumber> {
    match node.value {
        SourceNodeValue::Number(token) => SourceValue::Present(CatalogJsonNumber(token.to_owned())),
        _ => SourceValue::Invalid,
    }
}

pub(crate) fn array_len(node: &SourceNode<'_>) -> SourceValue<DecimalU64> {
    match &node.value {
        SourceNodeValue::Array(positions) => u64::try_from(positions.len())
            .map(DecimalU64::new)
            .map_or(SourceValue::Invalid, SourceValue::Present),
        _ => SourceValue::Invalid,
    }
}
