// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;
use std::collections::{HashMap, TryReserveError};
use std::io;

use p4kx_extraction::cryxml::{SourceDeclaration, SourceDeclarationKind};

use super::blend_reference_payload::{ExampleKind, PointKind};
use super::graph_field::GraphField;
use super::graph_source::GraphRole;
use crate::audio_control_payload::AtlPosition;
use crate::{MediaError, MediaWriter};

#[derive(Default)]
pub(super) struct BlendIndex<'source> {
    pub(super) real: Vec<Example>,
    pub(super) pseudo: Vec<Example>,
    aliases: HashMap<Cow<'source, str>, Vec<Alias<'source>>>,
    pub(super) queries: Vec<Query<'source>>,
    layout: OrdinalLayout,
}

#[derive(Default)]
struct OrdinalLayout {
    real: Option<AtlPosition>,
    pseudo: Option<AtlPosition>,
    version: Option<AtlPosition>,
    unsupported: bool,
}

#[derive(Clone, Copy)]
pub(super) struct Example {
    pub(super) index: u64,
    pub(super) position: AtlPosition,
    pub(super) kind: ExampleKind,
}

#[derive(Clone, Copy)]
pub(super) struct Alias<'source> {
    pub(super) spelling: &'source str,
    pub(super) target: Example,
    pub(super) ordinal: u64,
    pub(super) position: AtlPosition,
}

pub(super) struct Query<'source> {
    pub(super) kind: PointKind,
    pub(super) attribute: &'source str,
    pub(super) value: &'source str,
    pub(super) lexeme: Option<&'source str>,
    pub(super) indices: [u64; 2],
    pub(super) position: AtlPosition,
}

impl<'source> BlendIndex<'source> {
    pub(super) fn element(
        &mut self,
        role: Option<GraphRole>,
        parent: Option<GraphRole>,
        index: u64,
        position: AtlPosition,
        writer: &MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        match role {
            Some(GraphRole::ExampleList) => self.layout.table(ExampleKind::Real, position),
            Some(GraphRole::ExamplePseudo) => self.layout.table(ExampleKind::Pseudo, position),
            Some(GraphRole::Example) => push(
                &mut self.real,
                Example {
                    index,
                    position,
                    kind: ExampleKind::Real,
                },
                writer,
            )?,
            Some(GraphRole::Pseudo) => push(
                &mut self.pseudo,
                Example {
                    index,
                    position,
                    kind: ExampleKind::Pseudo,
                },
                writer,
            )?,
            None if matches!(
                parent,
                Some(GraphRole::ExampleList | GraphRole::ExamplePseudo)
            ) =>
            {
                self.layout.unsupported = true;
            }
            _ => {}
        }
        Ok(())
    }

    pub(super) fn declaration(
        &mut self,
        field: &GraphField,
        role: GraphRole,
        attribute: &SourceDeclaration<'source>,
        indices: [u64; 2],
        position: AtlPosition,
        writer: &MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let kind = match field {
            GraphField::FacePoint => PointKind::Face,
            GraphField::ExampleIndex => PointKind::Pseudo,
            GraphField::Alias => {
                return self.alias(role, attribute.value(), indices, position, writer);
            }
            GraphField::Version if matches!(role, GraphRole::BlendSpace) => {
                self.layout.version(attribute, position);
                return Ok(());
            }
            _ => return Ok(()),
        };
        push(
            &mut self.queries,
            Query {
                kind,
                attribute: attribute.qualified_name(),
                value: attribute.value(),
                lexeme: attribute.lexeme(),
                indices,
                position,
            },
            writer,
        )
    }

    fn alias(
        &mut self,
        role: GraphRole,
        value: &'source str,
        [element, ordinal]: [u64; 2],
        position: AtlPosition,
        writer: &MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        if value.is_empty() {
            return Ok(());
        }
        let example = match role {
            GraphRole::Example => self.real.last(),
            GraphRole::Pseudo => self.pseudo.last(),
            _ => None,
        }
        .filter(|example| example.index == element)
        .copied()
        .ok_or_else(|| MediaError::Contract("Blend-space alias has no original example".into()))?;
        self.aliases
            .try_reserve(1)
            .map_err(|cause| allocation(&cause, writer))?;
        push(
            self.aliases.entry(fold(value)).or_default(),
            Alias {
                spelling: value,
                target: example,
                ordinal,
                position,
            },
            writer,
        )
    }

    pub(super) fn aliases(&self, value: &str) -> &[Alias<'source>] {
        self.aliases
            .get(fold(value).as_ref())
            .map_or(&[], Vec::as_slice)
    }

    pub(super) const fn ordinal_layout_supported(&self) -> bool {
        self.layout.real.is_some() && !self.layout.unsupported
    }

    pub(super) fn ordinal_sites(&self, kind: PointKind) -> impl Iterator<Item = AtlPosition> {
        let pseudo = match kind {
            PointKind::Face => self.layout.pseudo,
            PointKind::Pseudo => None,
        };
        self.layout
            .version
            .into_iter()
            .chain(self.layout.real)
            .chain(pseudo)
    }

    pub(super) fn ordinal_target(&self, kind: PointKind, index: usize) -> Option<&Example> {
        if matches!(kind, PointKind::Face) && index >= self.real.len() {
            self.pseudo.get(index - self.real.len())
        } else {
            self.real.get(index)
        }
    }
}

impl OrdinalLayout {
    const fn table(&mut self, kind: ExampleKind, position: AtlPosition) {
        let site = match kind {
            ExampleKind::Real => &mut self.real,
            ExampleKind::Pseudo => &mut self.pseudo,
        };
        self.unsupported |= site.replace(position).is_some();
    }

    fn version(&mut self, attribute: &SourceDeclaration<'_>, site: AtlPosition) {
        if matches!(
            attribute.kind(),
            SourceDeclarationKind::Attribute { namespace: None }
        ) {
            self.unsupported |=
                self.version.replace(site).is_some() || !matches!(attribute.value(), "1" | "2");
        }
    }
}

pub(super) const fn accepts_alias(kind: PointKind, target: ExampleKind) -> bool {
    matches!(kind, PointKind::Face) || matches!(target, ExampleKind::Real)
}

fn fold(value: &str) -> Cow<'_, str> {
    if value.bytes().any(|byte| byte.is_ascii_uppercase()) {
        Cow::Owned(value.to_ascii_lowercase())
    } else {
        Cow::Borrowed(value)
    }
}

fn push<T>(values: &mut Vec<T>, value: T, writer: &MediaWriter<'_>) -> Result<(), MediaError> {
    values
        .try_reserve(1)
        .map_err(|cause| allocation(&cause, writer))?;
    values.push(value);
    Ok(())
}

fn allocation(cause: &TryReserveError, writer: &MediaWriter<'_>) -> MediaError {
    MediaError::Io {
        path: writer.root.clone(),
        source: io::Error::new(io::ErrorKind::OutOfMemory, cause.to_string()),
    }
}
