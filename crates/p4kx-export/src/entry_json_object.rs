// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Guid, SourceUseRole, SourceUses, SourceValue, SourcedValue};

use super::catalog_json_number::CatalogJsonNumber;
use crate::CatalogError;
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
        }))
    }

    pub(crate) fn field<T>(
        &self,
        name: &str,
        project: impl FnOnce(&SourceNode<'read>) -> SourceValue<T>,
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
                project(field)
            };
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
