// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;

use p4kx_extraction::{CancelToken, DecimalU64, SourceUse, SourceUseRole, SourceUses, SourceValue};

use crate::catalog_read::original::{EntryJson, JsonEvent};
use crate::shop_inventory_source::{JsonMember, JsonString, JsonValue, ShopInventorySource};
use crate::{CatalogError, ExportAccountingError};

/// The borrowed, owner-local source nodes before their JSON output is materialized.
pub(super) struct ParsedInventory<'read> {
    pub(super) root: usize,
    nodes: Vec<SourceNode<'read>>,
}

pub(super) struct SourceNode<'read> {
    pub(super) token: EntryJson<'read>,
    pub(super) payload: SourceUse,
    pub(super) value: SourceNodeValue<'read>,
}

pub(super) enum SourceNodeValue<'read> {
    Null,
    Boolean(bool),
    Number(&'read str),
    String(SourceString<'read>),
    Array(Vec<usize>),
    Object(Vec<SourceMember<'read>>),
}

pub(super) struct SourceMember<'read> {
    pub(super) name_token: EntryJson<'read>,
    pub(super) name: SourceString<'read>,
    pub(super) value: usize,
}

pub(super) enum SourceString<'read> {
    Text(Cow<'read, str>),
    Raw(&'read str),
}

#[derive(Clone, Copy)]
enum TextDomain {
    Technical,
    Unclassified,
}

impl SourceString<'_> {
    pub(super) fn text(&self) -> Option<&str> {
        match self {
            Self::Text(text) => Some(text),
            Self::Raw(_) => None,
        }
    }

    fn into_wire(self, domain: TextDomain) -> JsonString {
        match (self, domain) {
            (Self::Text(text), TextDomain::Technical) => JsonString::Public {
                text: text.into_owned(),
            },
            (Self::Text(text), TextDomain::Unclassified) => JsonString::GameText {
                text: text.into_owned(),
            },
            (Self::Raw(token), _) => JsonString::Raw {
                token: token.to_owned(),
            },
        }
    }
}

enum Open<'read> {
    Array(Vec<usize>),
    Object {
        members: Vec<SourceMember<'read>>,
        name: Option<EntryJson<'read>>,
    },
}

struct Parser<'read> {
    nodes: Vec<SourceNode<'read>>,
    open: Vec<Open<'read>>,
    root: Option<usize>,
}

impl<'read> ParsedInventory<'read> {
    pub(super) fn read(
        root: EntryJson<'read>,
        cancel: Option<&CancelToken>,
    ) -> Result<Self, CatalogError> {
        let mut parser = Parser {
            nodes: Vec::new(),
            open: Vec::new(),
            root: None,
        };
        for event in root.walk() {
            check_cancel(cancel)?;
            parser.event(event?)?;
        }
        if !parser.open.is_empty() {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        Ok(Self {
            root: parser
                .root
                .ok_or(ExportAccountingError::ConflictingSourceDatum)?,
            nodes: parser.nodes,
        })
    }

    pub(super) fn node(&self, index: usize) -> Result<&SourceNode<'read>, ExportAccountingError> {
        self.nodes
            .get(index)
            .ok_or(ExportAccountingError::ConflictingSourceDatum)
    }

    pub(super) fn into_wire(
        self,
        sources: &SourceUses,
        public_strings: &[usize],
        cancel: Option<&CancelToken>,
    ) -> Result<ShopInventorySource, CatalogError> {
        let mut values = Vec::new();
        values
            .try_reserve_exact(self.nodes.len())
            .map_err(CatalogError::JsonSourceAllocation)?;
        let mut public = public_strings.iter().copied().peekable();
        for (index, node) in self.nodes.into_iter().enumerate() {
            check_cancel(cancel)?;
            let domain = if public.next_if_eq(&index).is_some() {
                TextDomain::Technical
            } else {
                TextDomain::Unclassified
            };
            let value = node.value.into_wire(sources, domain)?;
            values.push(sources.value(value, std::slice::from_ref(&node.payload), true));
        }
        Ok(ShopInventorySource {
            root: address(self.root)?,
            values,
        })
    }
}

impl<'read> Parser<'read> {
    fn event(&mut self, event: JsonEvent<'read>) -> Result<(), CatalogError> {
        match event {
            JsonEvent::Array => push(&mut self.open, Open::Array(Vec::new())),
            JsonEvent::Object => push(
                &mut self.open,
                Open::Object {
                    members: Vec::new(),
                    name: None,
                },
            ),
            JsonEvent::Name(token) => {
                let Some(Open::Object { name, .. }) = self.open.last_mut() else {
                    return Err(ExportAccountingError::ConflictingSourceDatum.into());
                };
                if name.replace(token).is_some() {
                    return Err(ExportAccountingError::ConflictingSourceDatum.into());
                }
                Ok(())
            }
            JsonEvent::Value(token) => self.value(token, atom(token)?),
            JsonEvent::End(token) => {
                let value = match self.open.pop() {
                    Some(Open::Array(children)) => SourceNodeValue::Array(children),
                    Some(Open::Object {
                        members,
                        name: None,
                    }) => SourceNodeValue::Object(members),
                    _ => return Err(ExportAccountingError::ConflictingSourceDatum.into()),
                };
                self.value(token, value)
            }
        }
    }

    fn value(
        &mut self,
        token: EntryJson<'read>,
        value: SourceNodeValue<'read>,
    ) -> Result<(), CatalogError> {
        let index = self.nodes.len();
        let payload = token.observe(SourceUseRole::Payload)?;
        push(
            &mut self.nodes,
            SourceNode {
                token,
                payload,
                value,
            },
        )?;
        match self.open.last_mut() {
            Some(Open::Array(elements)) => push(elements, index),
            Some(Open::Object { members, name }) => {
                let name_token = name
                    .take()
                    .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
                push(
                    members,
                    SourceMember {
                        name_token,
                        name: source_string(name_token),
                        value: index,
                    },
                )
            }
            None if self.root.is_none() => {
                self.root = Some(index);
                Ok(())
            }
            None => Err(ExportAccountingError::ConflictingSourceDatum.into()),
        }
    }
}

impl SourceNodeValue<'_> {
    fn into_wire(
        self,
        sources: &SourceUses,
        domain: TextDomain,
    ) -> Result<SourceValue<JsonValue>, CatalogError> {
        let value = match self {
            Self::Null => return Ok(SourceValue::Null),
            Self::Boolean(value) => JsonValue::Boolean { value },
            Self::Number(token) => JsonValue::Number {
                token: token.to_owned(),
            },
            Self::String(value) => JsonValue::String {
                value: value.into_wire(domain),
            },
            Self::Array(elements) => JsonValue::Array {
                elements: convert(elements, address)?,
            },
            Self::Object(members) => JsonValue::Object {
                members: convert(members, |member| {
                    let used = member.name_token.observe(SourceUseRole::Payload)?;
                    Ok(JsonMember {
                        name: sources.value(
                            SourceValue::Present(member.name.into_wire(TextDomain::Technical)),
                            &[used],
                            true,
                        ),
                        value: address(member.value)?,
                    })
                })?,
            },
        };
        Ok(SourceValue::Present(value))
    }
}

fn atom(token: EntryJson<'_>) -> Result<SourceNodeValue<'_>, ExportAccountingError> {
    match token.raw() {
        "null" => Ok(SourceNodeValue::Null),
        "true" => Ok(SourceNodeValue::Boolean(true)),
        "false" => Ok(SourceNodeValue::Boolean(false)),
        raw if raw.starts_with('"') => Ok(SourceNodeValue::String(source_string(token))),
        _ => token
            .number()
            .map(SourceNodeValue::Number)
            .ok_or(ExportAccountingError::ConflictingSourceDatum),
    }
}

fn source_string(token: EntryJson<'_>) -> SourceString<'_> {
    match token.text() {
        Ok(Some(text)) => SourceString::Text(text),
        Ok(None) | Err(_) => SourceString::Raw(token.raw()),
    }
}

pub(super) fn address(index: usize) -> Result<DecimalU64, CatalogError> {
    let index = u64::try_from(index).map_err(|_| ExportAccountingError::ConflictingSourceDatum)?;
    Ok(DecimalU64::new(index))
}

pub(super) fn index(reference: DecimalU64) -> Result<usize, ExportAccountingError> {
    usize::try_from(reference.get()).map_err(|_| ExportAccountingError::ConflictingSourceDatum)
}

pub(super) fn push<T>(values: &mut Vec<T>, value: T) -> Result<(), CatalogError> {
    values
        .try_reserve(1)
        .map_err(CatalogError::JsonSourceAllocation)?;
    values.push(value);
    Ok(())
}

fn convert<T, U>(
    input: Vec<T>,
    mut project: impl FnMut(T) -> Result<U, CatalogError>,
) -> Result<Vec<U>, CatalogError> {
    let mut result = Vec::new();
    result
        .try_reserve_exact(input.len())
        .map_err(CatalogError::JsonSourceAllocation)?;
    for value in input {
        result.push(project(value)?);
    }
    Ok(result)
}

pub(super) fn check_cancel(cancel: Option<&CancelToken>) -> Result<(), CatalogError> {
    if cancel.is_some_and(CancelToken::is_cancelled) {
        return Err(CatalogError::Cancelled);
    }
    Ok(())
}
