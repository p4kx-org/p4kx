// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;

use p4kx_extraction::{CancelToken, SourceUse, SourceUseRole};

use crate::catalog_read::original::{EntryJson, JsonEvent};
use crate::{CatalogError, ExportAccountingError};

/// The borrowed, owner-local source nodes before their JSON output is materialized.
pub(crate) struct ParsedEntryJson<'read> {
    pub(crate) root: usize,
    pub(crate) nodes: Vec<SourceNode<'read>>,
}

pub(crate) struct SourceNode<'read> {
    pub(crate) token: EntryJson<'read>,
    pub(crate) payload: SourceUse,
    pub(crate) value: SourceNodeValue<'read>,
}

pub(crate) enum SourceNodeValue<'read> {
    Null,
    Boolean(bool),
    Number(&'read str),
    String(SourceString<'read>),
    Array(Vec<usize>),
    Object(Vec<SourceMember<'read>>),
}

pub(crate) struct SourceMember<'read> {
    pub(crate) name_token: EntryJson<'read>,
    pub(crate) name: SourceString<'read>,
    pub(crate) value: usize,
}

pub(crate) enum SourceString<'read> {
    Text(Cow<'read, str>),
    Raw(&'read str),
}

impl SourceString<'_> {
    pub(crate) fn text(&self) -> Option<&str> {
        match self {
            Self::Text(text) => Some(text),
            Self::Raw(_) => None,
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

impl<'read> ParsedEntryJson<'read> {
    pub(crate) fn read(
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

    pub(crate) fn node(&self, index: usize) -> Result<&SourceNode<'read>, ExportAccountingError> {
        self.nodes
            .get(index)
            .ok_or(ExportAccountingError::ConflictingSourceDatum)
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

pub(crate) fn member_uses(
    member: &SourceMember<'_>,
    node: &SourceNode<'_>,
) -> Result<[SourceUse; 2], CatalogError> {
    Ok([
        member.name_token.observe(SourceUseRole::Selector)?,
        node.payload.clone(),
    ])
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

pub(crate) fn push<T>(values: &mut Vec<T>, value: T) -> Result<(), CatalogError> {
    values
        .try_reserve(1)
        .map_err(CatalogError::JsonSourceAllocation)?;
    values.push(value);
    Ok(())
}

pub(crate) fn check_cancel(cancel: Option<&CancelToken>) -> Result<(), CatalogError> {
    if cancel.is_some_and(CancelToken::is_cancelled) {
        return Err(CatalogError::Cancelled);
    }
    Ok(())
}
