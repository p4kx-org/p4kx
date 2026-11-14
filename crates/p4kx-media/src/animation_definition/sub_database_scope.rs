// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::Arc;

use p4kx_extraction::cryxml::{SourceDocument, SourceElement, SourceReadOptions};
use p4kx_extraction::{InputScope, SourceUseRole};

use super::links;
use super::origins;
use super::payload::{self as wire, LinkKind};
use super::projection::{DefinitionProjection, Failure};
use super::sub_database_selectors_payload::{ScopeState, SelectorKind};
use crate::audio_control_payload::AtlPosition;
use crate::origin_capture::{OriginCapture, OriginSite};
use crate::origin_contributions::OriginContributions;
use crate::origin_input::MediaOriginInput;
use crate::xml_source_projection::{declaration_position, element_position};
use crate::{
    Identity, MediaError, MediaWriter, OriginalIdentifier, ReferenceResolution, Subresource,
    required_input,
};

pub(super) struct Scope {
    pub(super) state: ScopeState,
    pub(super) evidence: OriginContributions,
    names: BTreeMap<String, Vec<Candidate>>,
}

pub(super) struct Candidate {
    pub(super) source: Arc<DefinitionSource>,
    pub(super) name: String,
    pub(super) position: AtlPosition,
    pub(super) name_position: AtlPosition,
}

pub(super) struct DefinitionSource {
    pub(super) target: Identity,
    pub(super) input: MediaOriginInput,
}

struct Survey<'a, 'context, 'source, 'writer> {
    projection: &'a DefinitionProjection<'context, 'source>,
    writer: &'a mut MediaWriter<'writer>,
    evidence: OriginCapture,
    names: BTreeMap<String, Vec<Candidate>>,
    visited: BTreeSet<usize>,
    imports: VecDeque<Import>,
    complete: bool,
}

impl Scope {
    pub(super) fn read(
        projection: &DefinitionProjection<'_, '_>,
        root: SourceElement<'_>,
        kind: SelectorKind,
        writer: &mut MediaWriter<'_>,
    ) -> Result<Self, Failure> {
        let name = match kind {
            SelectorKind::Tags => "TagDef",
            SelectorKind::FragmentName => "FragDef",
        };
        let mut evidence = writer.capture_origins();
        evidence.include(
            &origins::position(
                projection.input,
                element_position(&root.position())?,
                SourceUseRole::Selector,
                writer,
            )?,
            writer,
        )?;
        let mut paths = Vec::new();
        for attribute in root.attributes().filter(|attribute| {
            attribute.namespace().is_none() && attribute.name().eq_ignore_ascii_case(name)
        }) {
            writer.checkpoint()?;
            evidence.include(
                &origins::position(
                    projection.input,
                    declaration_position(attribute.position())?,
                    SourceUseRole::Selector,
                    writer,
                )?,
                writer,
            )?;
            paths.push(attribute.value());
        }
        let mut survey = Survey {
            projection,
            writer,
            evidence,
            names: BTreeMap::new(),
            visited: BTreeSet::new(),
            imports: VecDeque::new(),
            complete: true,
        };
        let state = match paths.as_slice() {
            [] => ScopeState::MissingDefinitionSelector,
            [path] => survey.follow(path)?,
            _ => ScopeState::DuplicateDefinitionSelector,
        };
        let state = if matches!(state, ScopeState::Complete) {
            survey.imports()?
        } else {
            state
        };
        if !matches!(state, ScopeState::Complete) {
            survey.evidence.include(
                &OriginContributions::unknown(&survey.writer.origins.owner),
                survey.writer,
            )?;
        }
        Ok(Self {
            state,
            evidence: survey.evidence.finish(),
            names: survey.names,
        })
    }

    pub(super) fn candidates(&self, name: &str) -> &[Candidate] {
        self.names
            .get(&name.to_ascii_lowercase())
            .map_or(&[], Vec::as_slice)
    }
}

impl Survey<'_, '_, '_, '_> {
    fn follow(&mut self, path: &str) -> Result<ScopeState, Failure> {
        self.work()?;
        let Some(canonical) = links::target_path(LinkKind::TagDefinition, path) else {
            return Ok(ScopeState::InvalidDefinitionPath);
        };
        let directory = self.projection.context.directory;
        let observed = directory.lookup_input_observed(
            OriginalIdentifier::new_with_path(path),
            &canonical,
            None,
            self.writer,
        )?;
        self.evidence.include(&observed.inventory, self.writer)?;
        for input in observed
            .evidence
            .retained
            .iter()
            .chain(observed.evidence.discarded.iter())
        {
            self.evidence.add(
                input,
                InputScope::ArchiveDirectoryMetadata,
                OriginSite::new("archive-entry", &[], "reference-candidate"),
                SourceUseRole::Selector,
                self.writer,
            )?;
        }
        match observed.lookup.reference.resolution() {
            ReferenceResolution::Missing => return Ok(ScopeState::MissingDefinition),
            ReferenceResolution::Ambiguous { .. } => return Ok(ScopeState::AmbiguousDefinition),
            ReferenceResolution::NotSelected { .. } => return Ok(ScopeState::NotSelected),
            ReferenceResolution::Unavailable { .. } => return Ok(ScopeState::Unavailable),
            ReferenceResolution::Resolved { .. } => {}
        }
        let position = observed.lookup.input.ok_or_else(contract)?.position;
        self.read_definition(position)
    }

    fn read_definition(&mut self, position: usize) -> Result<ScopeState, Failure> {
        if !self.visited.insert(position) {
            return Ok(ScopeState::Complete);
        }
        let directory = self.projection.context.directory;
        let archive = directory.index.archive;
        let entry = archive.entries().get(position).ok_or_else(contract)?;
        if entry.is_encrypted() && !archive.has_key() {
            return Ok(ScopeState::Unreadable);
        }
        let location = Subresource::new(wire::SUB_DATABASE_SELECTOR, Vec::new());
        let mut evidence = self.writer.capture_origins();
        let read = required_input::with_reference_input(
            directory,
            entry,
            &location,
            self.writer,
            |verified, writer| {
                let bytes = verified.read_payload()?;
                let input = verified.origin_input(writer)?;
                evidence.include(&origins::file(&input, bytes.len() as u64, writer)?, writer)?;
                let source = Arc::new(DefinitionSource {
                    target: verified.receipt().source.identity.clone(),
                    input,
                });
                definition(&bytes, &source, self.projection, writer)
            },
        );
        let definition = match read {
            Ok(definition) => definition,
            Err(error) if unreadable(&error) => return Ok(ScopeState::Unreadable),
            Err(error) => return Err(error.into()),
        };
        self.evidence.include(&evidence.finish(), self.writer)?;
        let Some(definition) = definition else {
            return Ok(ScopeState::InvalidDefinition);
        };
        self.complete &= definition.complete;
        self.imports.extend(definition.imports);
        for candidate in definition.names {
            self.names
                .entry(candidate.name.to_ascii_lowercase())
                .or_default()
                .push(candidate);
        }
        Ok(ScopeState::Complete)
    }

    fn imports(&mut self) -> Result<ScopeState, Failure> {
        while let Some(import) = self.imports.pop_front() {
            self.evidence.include(
                &origins::position(
                    &import.source.input,
                    import.position,
                    SourceUseRole::Selector,
                    self.writer,
                )?,
                self.writer,
            )?;
            if !matches!(self.follow(&import.path)?, ScopeState::Complete) {
                self.complete = false;
            }
        }
        Ok(if self.complete {
            ScopeState::Complete
        } else {
            ScopeState::IncompleteImports
        })
    }

    fn work(&self) -> Result<(), MediaError> {
        metadata(self.projection, self.writer)
    }
}

pub(super) fn metadata(
    projection: &DefinitionProjection<'_, '_>,
    writer: &MediaWriter<'_>,
) -> Result<(), MediaError> {
    projection.context.directory.input_state()?.budget.metadata(
        0,
        1,
        writer.limits(),
        writer.checkpoint,
    )
}

fn unreadable(error: &MediaError) -> bool {
    match error {
        MediaError::Source { cause, .. } => unreadable(cause),
        MediaError::Extraction { cause, .. } => matches!(
            cause.as_ref(),
            p4kx_extraction::ExtractionError::Archive(_)
                | p4kx_extraction::ExtractionError::Read(_)
                | p4kx_extraction::ExtractionError::ChecksumMismatch { .. }
        ),
        _ => false,
    }
}

struct Import {
    path: String,
    source: Arc<DefinitionSource>,
    position: AtlPosition,
}

#[derive(Default)]
struct Definition {
    imports: Vec<Import>,
    names: Vec<Candidate>,
    complete: bool,
}

fn definition(
    bytes: &[u8],
    source: &Arc<DefinitionSource>,
    projection: &DefinitionProjection<'_, '_>,
    writer: &MediaWriter<'_>,
) -> Result<Option<Definition>, MediaError> {
    let Ok(document) = SourceDocument::parse(bytes, SourceReadOptions::default()) else {
        return Ok(None);
    };
    let root = document.root();
    if !named(root, "TagDefinition") || document.is_lossy() {
        return Ok(None);
    }
    let mut result = Definition {
        complete: true,
        ..Definition::default()
    };
    for section in root.elements() {
        metadata(projection, writer)?;
        if named(section, "Imports") {
            for import in section.elements().filter(|node| named(*node, "Import")) {
                metadata(projection, writer)?;
                let mut attributes = import.attributes().filter(|attribute| {
                    attribute.namespace().is_none() && attribute.name() == "filename"
                });
                match (attributes.next(), attributes.next()) {
                    (Some(path), None) => result.imports.push(Import {
                        path: path.value().to_owned(),
                        source: Arc::clone(source),
                        position: declaration_position(path.position()).map_err(|_| contract())?,
                    }),
                    _ => result.complete = false,
                }
            }
        } else if named(section, "Tags") {
            result.tags(section, source, projection, writer)?;
        }
    }
    Ok(Some(result))
}

impl Definition {
    fn tags(
        &mut self,
        section: SourceElement<'_>,
        source: &Arc<DefinitionSource>,
        projection: &DefinitionProjection<'_, '_>,
        writer: &MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        for member in section.elements() {
            metadata(projection, writer)?;
            if named(member, "Tag") {
                self.tag(member, source)?;
            } else if named(member, "Group") {
                for tag in member.elements().filter(|node| named(*node, "Tag")) {
                    metadata(projection, writer)?;
                    self.tag(tag, source)?;
                }
            }
        }
        Ok(())
    }

    fn tag(
        &mut self,
        tag: SourceElement<'_>,
        source: &Arc<DefinitionSource>,
    ) -> Result<(), MediaError> {
        let position = element_position(&tag.position()).map_err(|_| contract())?;
        let mut attributes = tag
            .attributes()
            .filter(|attribute| attribute.namespace().is_none() && attribute.name() == "name");
        let (Some(name), None) = (attributes.next(), attributes.next()) else {
            self.complete = false;
            return Ok(());
        };
        let name_position = declaration_position(name.position()).map_err(|_| contract())?;
        self.names.push(Candidate {
            source: Arc::clone(source),
            name: name.value().to_owned(),
            position,
            name_position,
        });
        Ok(())
    }
}

fn named(node: SourceElement<'_>, name: &str) -> bool {
    node.namespace().is_none() && node.name() == name
}

fn contract() -> MediaError {
    MediaError::Contract("SubADB scope lost its original definition coordinates".into())
}
