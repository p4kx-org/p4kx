// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;
use std::path::Path;

use p4kx_extraction::cryxml::{
    SourceContent, SourceContents, SourceDeclaration, SourceDocument, SourceElement, SourceSyntax,
};
use p4kx_extraction::sound::radar_audio::RadarAudioElement;
use p4kx_extraction::{DecimalU64, SourceUseRole};

use crate::audio_control_fields::{AtlFamily, AtlScalar};
use crate::audio_control_graph::Site;
use crate::audio_control_payload::{self as wire, AtlPayload, AtlPosition};
use crate::audio_control_reference::NameDeclaration;
use crate::audio_control_source::AudioControlSource;
use crate::audio_control_source::cig_audio_field::CigAudioFieldRecord;
use crate::audio_control_source::cig_audio_role::CigAudioRole;
use crate::xml_source_projection::{content_position, element_position, source_content};
use crate::{MediaError, Subresource, origin_error};

use super::Failure;
use super::declaration::ObservedDeclaration;

pub(super) trait Sink {
    fn work(&mut self) -> Result<(), MediaError>;
    fn path(&self) -> &Path;
    fn document(&mut self, payload: wire::AtlDocument) -> Result<(), MediaError>;
    fn record(
        &mut self,
        position: AtlPosition,
        field: &'static str,
        role: SourceUseRole,
        subresource: &Subresource,
        payload: &AtlPayload<'_>,
    ) -> Result<(), MediaError>;
    fn name(&mut self, declaration: NameDeclaration<'_>) -> Result<(), MediaError>;
}

struct Frame<'document> {
    parent: Option<u64>,
    children: SourceContents<'document>,
    sibling: u64,
    preload: Option<u64>,
    radar: Option<RadarAudioElement>,
    cig_audio: Option<CigAudioRole>,
}

struct ElementRoute {
    location: wire::AtlLocation,
    index: u64,
    preload: Option<u64>,
    radar: Option<RadarAudioElement>,
    cig_audio: Option<CigAudioRole>,
}

#[derive(Clone, Copy)]
struct Definition {
    family: Option<AtlFamily>,
    preload: Option<u64>,
    radar: Option<RadarAudioElement>,
    cig_audio: Option<CigAudioRole>,
}

#[derive(Clone, Copy)]
enum Layout {
    Xml,
    Atl,
    Radar(RadarAudioElement),
    CigAudio(CigAudioRole),
}

impl Layout {
    fn detect(kind: AudioControlSource, root: SourceElement<'_>) -> Self {
        match kind {
            AudioControlSource::Atl if root.name() == "ATLConfig" => Self::Atl,
            AudioControlSource::Atl => RadarAudioElement::root(root).map_or(Self::Xml, Self::Radar),
            AudioControlSource::CigAudio => {
                CigAudioRole::root(root).map_or(Self::Xml, Self::CigAudio)
            }
        }
    }

    const fn radar(self) -> Option<RadarAudioElement> {
        match self {
            Self::Radar(root) => Some(root),
            _ => None,
        }
    }

    const fn cig_audio(self) -> Option<CigAudioRole> {
        match self {
            Self::CigAudio(root) => Some(root),
            _ => None,
        }
    }
}

pub(super) fn project(
    source: &SourceDocument<'_>,
    kind: AudioControlSource,
    entry: usize,
    sink: &mut impl Sink,
) -> Result<(), Failure> {
    let layout = Layout::detect(kind, source.root());
    sink.document(wire::AtlDocument {
        format: source.format().into(),
        root_is_atl_config: matches!(layout, Layout::Atl),
        repaired_string_bytes: source.is_lossy(),
    })?;
    Walk { entry, sink }.tree(source, layout)
}

struct Walk<'sink, S> {
    entry: usize,
    sink: &'sink mut S,
}

impl<S: Sink> Walk<'_, S> {
    fn tree(&mut self, source: &SourceDocument<'_>, layout: Layout) -> Result<(), Failure> {
        let mut stack = Vec::new();
        reserve(&mut stack, self.sink.path())?;
        stack.push(Frame {
            parent: None,
            children: source.content(),
            sibling: 0,
            preload: None,
            radar: None,
            cig_audio: None,
        });
        let mut next = 0_u64;
        while let Some(frame) = stack.last_mut() {
            let Some(item) = frame.children.next() else {
                stack.pop();
                continue;
            };
            self.sink.work()?;
            let location = wire::AtlLocation {
                parent: frame.parent.map(DecimalU64::new),
                sibling_ordinal: DecimalU64::new(frame.sibling),
            };
            frame.sibling = advance(frame.sibling)?;
            let index = next;
            next = advance(next)?;
            match item? {
                SourceContent::Element(element) => {
                    let (radar, cig_audio) = if frame.parent.is_none() {
                        (layout.radar(), layout.cig_audio())
                    } else {
                        (
                            frame.radar.and_then(|parent| parent.child(element)),
                            frame.cig_audio.and_then(|parent| parent.child(element)),
                        )
                    };
                    let route = ElementRoute {
                        location,
                        index,
                        preload: frame.preload,
                        radar,
                        cig_audio,
                    };
                    let child = self.element(element, route, matches!(layout, Layout::Atl))?;
                    reserve(&mut stack, self.sink.path())?;
                    stack.push(child);
                }
                other => self.content(&other, location, index)?,
            }
        }
        Ok(())
    }

    fn element<'document>(
        &mut self,
        element: SourceElement<'document>,
        route: ElementRoute,
        typed: bool,
    ) -> Result<Frame<'document>, Failure> {
        let ElementRoute {
            location,
            index,
            preload,
            radar,
            cig_audio,
        } = route;
        let syntax = element.syntax()?;
        let position = element_position(&element.position())?;
        self.payload(
            position,
            "element",
            &Subresource::new(wire::ELEMENT, vec![index]),
            &AtlPayload::Element(wire::AtlElement {
                qualified_name: syntax.qualified_name(),
                namespace: element.namespace(),
                location,
                position,
            }),
        )?;
        let family = if typed {
            AtlFamily::from_tag(element.name())
        } else {
            None
        };
        let preload = if family == Some(AtlFamily::PreloadRequest) {
            Some(index)
        } else {
            preload
        };
        let definition = Definition {
            family,
            preload,
            radar,
            cig_audio,
        };
        self.declarations(&syntax, definition, position, index)?;
        if let Some(text) = syntax.binary_text() {
            let position = content_position(text.position())?;
            self.payload(
                position,
                "content",
                &Subresource::new(wire::BINARY_TEXT, vec![index]),
                &AtlPayload::BinaryText(wire::AtlBinaryText {
                    position,
                    value: text.value(),
                }),
            )?;
        }
        Ok(Frame {
            parent: Some(index),
            children: syntax.content()?,
            sibling: 0,
            preload,
            radar,
            cig_audio,
        })
    }

    fn declarations(
        &mut self,
        syntax: &SourceSyntax<'_>,
        definition: Definition,
        position: AtlPosition,
        index: u64,
    ) -> Result<(), Failure> {
        if let Some(family) = definition.family {
            self.payload(
                position,
                "element",
                &Subresource::new(wire::DEFINITION, vec![index]),
                &AtlPayload::Definition(wire::AtlDefinition { family }),
            )?;
        }
        if let Some(radar) = definition.radar {
            self.payload(
                position,
                "element",
                &Subresource::new(wire::RADAR_DEFINITION, vec![index]),
                &AtlPayload::RadarDefinition(radar),
            )?;
        }
        if let Some(role) = definition.cig_audio {
            self.payload(
                position,
                "element",
                &Subresource::new(wire::CIG_AUDIO_DEFINITION, vec![index]),
                &AtlPayload::CigAudioDefinition(wire::CigAudioDefinition { role }),
            )?;
        }
        for (ordinal, declaration) in syntax.declarations().enumerate() {
            self.sink.work()?;
            self.declaration(&declaration?, definition, [index, ordinal as u64])?;
        }
        Ok(())
    }

    fn declaration(
        &mut self,
        declaration: &SourceDeclaration<'_>,
        definition: Definition,
        indices: [u64; 2],
    ) -> Result<(), Failure> {
        let observed = ObservedDeclaration::read(declaration, definition.family)?;
        let position = observed.original.position;
        self.payload(
            position,
            "declaration",
            &Subresource::new(wire::DECLARATION, indices.to_vec()),
            &AtlPayload::Declaration(observed.original),
        )?;
        if let Some(field) = definition
            .radar
            .and_then(|radar| radar.attribute(declaration))
        {
            self.sink.record(
                position,
                "declaration",
                SourceUseRole::Conversion,
                &Subresource::new(wire::RADAR_FIELD, indices.to_vec()),
                &AtlPayload::RadarField(field),
            )?;
        }
        if let Some(field) = definition
            .cig_audio
            .and_then(|role| CigAudioFieldRecord::read(role, declaration))
        {
            self.sink.record(
                position,
                "declaration",
                SourceUseRole::Conversion,
                &Subresource::new(wire::CIG_AUDIO_FIELD, indices.to_vec()),
                &AtlPayload::CigAudioField(field),
            )?;
        }
        if let Some(typed) = observed.typed {
            let short_id = match &typed.record.typed_value {
                AtlScalar::WwiseEvent { id, .. } => Some(*id),
                _ => None,
            };
            let site = Site {
                entry: self.entry,
                indices,
                position,
            };
            self.sink.name(NameDeclaration {
                family: typed.family,
                field: typed.record.field,
                name: declaration.value(),
                short_id,
                site,
                preload: definition.preload,
            })?;
            self.sink.record(
                position,
                "declaration",
                SourceUseRole::Conversion,
                &Subresource::new(wire::FIELD, indices.to_vec()),
                &AtlPayload::Field(typed.record),
            )?;
        }
        Ok(())
    }

    fn content(
        &mut self,
        content: &SourceContent<'_>,
        location: wire::AtlLocation,
        index: u64,
    ) -> Result<(), Failure> {
        let (position, content) = source_content(content)?;
        self.payload(
            position,
            "content",
            &Subresource::new(wire::CONTENT, vec![index]),
            &AtlPayload::Content(wire::AtlContent {
                location,
                position,
                content,
            }),
        )?;
        Ok(())
    }

    fn payload(
        &mut self,
        position: AtlPosition,
        field: &'static str,
        subresource: &Subresource,
        payload: &AtlPayload<'_>,
    ) -> Result<(), MediaError> {
        self.sink.record(
            position,
            field,
            SourceUseRole::Payload,
            subresource,
            payload,
        )
    }
}

fn reserve<T>(stack: &mut Vec<T>, path: &Path) -> Result<(), MediaError> {
    stack.try_reserve(1).map_err(|cause| MediaError::Io {
        path: path.to_owned(),
        source: io::Error::new(io::ErrorKind::OutOfMemory, cause),
    })
}

fn advance(value: u64) -> Result<u64, MediaError> {
    value.checked_add(1).ok_or_else(origin_error::counter)
}
