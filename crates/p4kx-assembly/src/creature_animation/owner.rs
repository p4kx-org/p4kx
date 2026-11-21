// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::ops::Range;

use p4kx_extraction::{
    ArchiveAnimationRead, ArchiveClipEvidence, DataForgeOrigin, EntryOrigin, InputScope,
    SourceEvidenceError, SourceInput, SourceInventory, SourceLocation, SourceScope, SourceUse,
    SourceUseRole, SourceUses, SourceValue, SourcedValue,
};

use p4kx_model::AnimationClipCoordinates;

use super::component::AnimationDatabase;
use super::database::{NameSelector, Route};

pub(super) struct ParameterRoute {
    pub(super) skeleton: String,
    pub(super) cdf: Option<CdfModel>,
    pub(super) params: Option<(EntryOrigin, u64)>,
}

pub(super) struct CdfModel {
    pub(super) input: Option<EntryOrigin>,
    pub(super) field: Option<SourceLocation>,
}

pub(super) struct OwnerInputs<'a> {
    pub(super) forge: Option<&'a DataForgeOrigin>,
    pub(super) database: &'a AnimationDatabase,
    pub(super) geometry: &'a str,
    pub(super) geometry_field: Option<SourceLocation>,
    pub(super) parameters: &'a ParameterRoute,
}

pub(super) struct SelectionInputs<'a> {
    pub(super) names: &'a [NameSelector],
    pub(super) clips: &'a ArchiveAnimationRead,
    pub(super) complete: bool,
}

/// Authored ownership and physical clips observed by creature animation selection.
#[derive(Debug)]
#[non_exhaustive]
pub struct CreatureAnimationSourceReport {
    /// Authored Entity and skeleton-parameter route.
    pub owner: CreatureAnimationOwner,
    /// Physical clips in the same order as the selected animation set.
    pub clips: Vec<CreatureAnimationClipSource>,
    /// Whether the preview completed its source scan without an observed omission.
    pub selection_complete: bool,
    /// Original physical sources and immediate field-use references.
    pub source_inventory: SourceInventory,
}

/// Authored selectors that tie a creature's animation graph to its parameter scope.
#[derive(Debug)]
#[non_exhaustive]
pub struct CreatureAnimationOwner {
    /// Original animated-character component field.
    pub database: SourcedValue<String>,
    /// Geometry used when selecting the animation parameters.
    pub geometry: SourcedValue<String>,
    /// Original CDF Model File or selected direct geometry.
    pub skeleton: SourcedValue<String>,
    /// Physical CHRPARAMS Entry admitted by the derived sibling lookup.
    pub parameters: SourcedValue<String>,
}

/// Original clip and the ADB selectors admissible in the observed creature scope.
#[derive(Debug)]
#[non_exhaustive]
pub struct CreatureAnimationClipSource {
    /// Path of the exact physical file whose clip was read.
    pub entry: SourcedValue<String>,
    /// Original clip ordinal within that file.
    pub ordinal: SourcedValue<usize>,
    /// Original ADB animation-name occurrences that match the selected clip.
    pub graph_selectors: Vec<SourcedValue<String>>,
}

pub(super) fn report(
    owner: &OwnerInputs<'_>,
    selection: &SelectionInputs<'_>,
) -> Result<CreatureAnimationSourceReport, SourceEvidenceError> {
    let sources = SourceUses::with_compact_use_refs();
    let (owner, uses) = owner.project(&sources)?;
    let selectors = ClipSelection::new(&sources, selection.names);
    let mut clips = Vec::with_capacity(selection.clips.set().animations.len());
    for index in 0..selection.clips.set().animations.len() {
        clips.push(selectors.record(
            selection.clips.matched_specs(index),
            selection.clips.archive_source(index),
            selection.clips.clip_evidence(index),
            &uses,
        )?);
    }
    Ok(CreatureAnimationSourceReport {
        owner,
        clips,
        selection_complete: selection.complete,
        source_inventory: sources.freeze()?,
    })
}

struct OwnerProof {
    uses: Vec<SourceUse>,
    complete: bool,
}

impl OwnerInputs<'_> {
    fn project(
        &self,
        sources: &SourceUses,
    ) -> Result<(CreatureAnimationOwner, OwnerProof), SourceEvidenceError> {
        let forge = match self.forge {
            Some(DataForgeOrigin::Archive(input)) => {
                Some(observe(sources, input, InputScope::Payload)?)
            }
            _ => None,
        };
        let database = at(forge.as_ref(), Some(self.database.location()))?;
        let geometry = at(forge.as_ref(), self.geometry_field.clone())?;
        let skeleton_use = if let Some(cdf) = &self.parameters.cdf {
            let scope = cdf
                .input
                .as_ref()
                .map(|input| observe(sources, input, InputScope::Payload))
                .transpose()?;
            at(scope.as_ref(), cdf.field.clone())?
        } else {
            geometry.clone()
        };
        let p = self.parameters.params.as_ref();
        let params = p.map(|(input, _)| path_use(sources, input)).transpose()?;
        let parameter_uses: Vec<_> = skeleton_use.iter().chain(params.iter()).cloned().collect();
        let parameter_payload = p
            .map(|(input, length)| {
                observe(sources, input, InputScope::Payload)?.at(
                    SourceLocation::Subresource {
                        namespace: "characterParameters".into(),
                        indices: vec![0, *length],
                        field: "animationSources".into(),
                    },
                    SourceUseRole::Selector,
                )
            })
            .transpose()?;
        let owner = CreatureAnimationOwner {
            database: text(sources, self.database.authored(), database.as_slice()),
            geometry: text(sources, self.geometry, geometry.as_slice()),
            skeleton: text(sources, &self.parameters.skeleton, skeleton_use.as_slice()),
            parameters: p.map_or_else(
                || sources.value(SourceValue::Unreadable, &parameter_uses, false),
                |(input, _)| {
                    sources.value(
                        SourceValue::Present(input.entry().path().to_owned()),
                        &parameter_uses,
                        skeleton_use.is_some(),
                    )
                },
            ),
        };
        let complete = database.is_some()
            && geometry.is_some()
            && skeleton_use.is_some()
            && parameter_payload.is_some();
        let uses = database
            .into_iter()
            .chain(geometry)
            .chain(skeleton_use)
            .chain(params)
            .chain(parameter_payload)
            .collect();
        Ok((owner, OwnerProof { uses, complete }))
    }
}

struct ClipSelection<'a> {
    sources: &'a SourceUses,
    selectors: HashMap<String, Vec<(usize, &'a NameSelector)>>,
}

impl<'a> ClipSelection<'a> {
    fn new(sources: &'a SourceUses, selectors: &'a [NameSelector]) -> Self {
        let mut by_name: HashMap<String, Vec<(usize, &NameSelector)>> = HashMap::new();
        for (ordinal, selector) in selectors.iter().enumerate() {
            by_name
                .entry(selector.name.to_ascii_lowercase())
                .or_default()
                .push((ordinal, selector));
        }
        Self {
            sources,
            selectors: by_name,
        }
    }

    fn record(
        &self,
        matched_specs: &[String],
        input: Option<&EntryOrigin>,
        clip: Option<&ArchiveClipEvidence>,
        owner: &OwnerProof,
    ) -> Result<CreatureAnimationClipSource, SourceEvidenceError> {
        let path = input
            .map(|input| path_use(self.sources, input))
            .transpose()?;
        let entry = input.map_or_else(
            || self.sources.value(SourceValue::Unreadable, &[], false),
            |input| text(self.sources, input.entry().path(), path.as_slice()),
        );
        let physical = input.zip(clip);
        let ordinal_use = physical
            .and_then(|(input, clip)| {
                Some((input, clip.ordinal, clip.coordinates.as_ref()?.flags()))
            })
            .map(|(input, ordinal, bytes)| clip_field(self.sources, input, ordinal, bytes, "flags"))
            .transpose()?;
        let selection_use = physical
            .map(|(input, clip)| selection_use(self.sources, input, clip))
            .transpose()?;
        let ordinal = clip.filter(|_| input.is_some()).map_or_else(
            || self.sources.value(SourceValue::Unreadable, &[], false),
            |clip| {
                self.sources.value(
                    SourceValue::Present(clip.ordinal),
                    ordinal_use.as_slice(),
                    ordinal_use.is_some(),
                )
            },
        );
        let mut graph_selectors = Vec::new();
        if let Some(clip_use) = &selection_use {
            for selector in self.matching_selectors(matched_specs) {
                let scope = observe(self.sources, &selector.input, InputScope::Payload)?;
                let location = scope.at(selector.position.clone(), SourceUseRole::Selector)?;
                let mut actual = owner.uses.clone();
                actual.extend([location, clip_use.clone()]);
                actual.extend(ordinal_use.iter().cloned());
                let complete = add_route(self.sources, &selector.route, &mut actual)?;

                graph_selectors.push(self.sources.value(
                    SourceValue::Present(selector.name.clone()),
                    &actual,
                    owner.complete && complete && ordinal_use.is_some(),
                ));
            }
        }
        Ok(CreatureAnimationClipSource {
            entry,
            ordinal,
            graph_selectors,
        })
    }

    fn matching_selectors(&self, matched_specs: &[String]) -> Vec<&NameSelector> {
        let mut selected: Vec<_> = matched_specs
            .iter()
            .flat_map(|name| {
                self.selectors
                    .get(&name.to_ascii_lowercase())
                    .into_iter()
                    .flatten()
                    .copied()
            })
            .collect();
        selected.sort_unstable_by_key(|(ordinal, _)| *ordinal);
        selected.dedup_by_key(|(ordinal, _)| *ordinal);
        selected.into_iter().map(|(_, selector)| selector).collect()
    }
}

fn add_route(
    sources: &SourceUses,
    mut route: &Route,
    actual: &mut Vec<SourceUse>,
) -> Result<bool, SourceEvidenceError> {
    let mut complete = true;
    while let Route::Nested(link) = route {
        if let Some(selector) = &link.selector {
            actual.push(
                observe(sources, &selector.input, InputScope::Payload)?
                    .at(selector.position.clone(), SourceUseRole::Selector)?,
            );
        } else {
            complete = false;
        }
        route = &link.parent;
    }
    Ok(complete)
}

fn selection_use(
    sources: &SourceUses,
    input: &EntryOrigin,
    clip: &ArchiveClipEvidence,
) -> Result<SourceUse, SourceEvidenceError> {
    clip.coordinates
        .as_ref()
        .and_then(AnimationClipCoordinates::database_path)
        .map_or_else(
            || path_use(sources, input),
            |bytes| clip_field(sources, input, clip.ordinal, bytes, "databasePath"),
        )
}

fn clip_field(
    sources: &SourceUses,
    input: &EntryOrigin,
    ordinal: usize,
    bytes: Range<u64>,
    field: &str,
) -> Result<SourceUse, SourceEvidenceError> {
    observe(sources, input, InputScope::Payload)?.at(
        SourceLocation::Subresource {
            namespace: "animationClipMetadata".into(),
            indices: vec![ordinal as u64, bytes.start, bytes.end],
            field: field.into(),
        },
        SourceUseRole::Selector,
    )
}

fn observe(
    uses: &SourceUses,
    input: &EntryOrigin,
    scope: InputScope,
) -> Result<SourceScope, SourceEvidenceError> {
    uses.observe(SourceInput::Archive(input), scope)
}

fn at(
    source: Option<&SourceScope>,
    location: Option<SourceLocation>,
) -> Result<Option<SourceUse>, SourceEvidenceError> {
    source
        .zip(location)
        .map(|(source, location)| source.at(location, SourceUseRole::Selector))
        .transpose()
}

fn path_use(uses: &SourceUses, input: &EntryOrigin) -> Result<SourceUse, SourceEvidenceError> {
    observe(uses, input, InputScope::ArchiveDirectoryMetadata)?.at(
        SourceLocation::Metadata {
            field: "path".into(),
        },
        SourceUseRole::Selector,
    )
}

fn text(uses: &SourceUses, value: &str, actual: &[SourceUse]) -> SourcedValue<String> {
    uses.value(
        SourceValue::Present(value.to_owned()),
        actual,
        !actual.is_empty(),
    )
}
