// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use p4kx_assembly::{BodyVariant, Entities};
use p4kx_extraction::{
    Archive, ArchiveWatch, CancelToken, DataForgeEntry, DataForgeOrigin, EntryOrigin, FieldOrigin,
    InputScope, PathKeys, PhysicalSourceRef, SourceEvidenceError, SourceInput, SourceInventory,
    SourceScope, SourceUses, SourcedValue,
};
use schemars::JsonSchema;
use serde::Serialize;

use crate::player_adb_declarations::AdbDeclarations;
use crate::player_source_body::{self, Body};
use crate::player_source_loadout::{self, Loadout};
use crate::player_source_motion::{ControllerTopology, Motion, RigCandidate, read_rig_candidates};
use crate::player_tag_definitions::{AdbTagUses, TagDefinitions, WeaponMotionTag};
use crate::player_weapon_motion_sources::{self, WeaponMotionSources};
use crate::soc_scene_class_resources::Resources;
use crate::soc_scene_xml::XmlDocuments;

use super::{PlayerSourceError, check, index_error};

/// Verified bytes and document-local identity observed during Player XML capture.
#[derive(Debug)]
#[non_exhaustive]
pub struct PlayerDocument<'a> {
    /// Index referenced by the corresponding Player records.
    pub index: usize,
    /// Source shared with all field-use receipts in this capture.
    pub source: PhysicalSourceRef,
    /// Exact original archive entry and containing context.
    pub original: &'a EntryOrigin,
    /// Root element evidence of the selected XML source.
    pub origin: FieldOrigin,
    /// Original XML encoding interpreted by the source reader.
    pub format: &'static str,
    /// Whether source string repair occurred during decoding.
    pub lossy: bool,
    /// Exact decoded bytes of the physical Archive member.
    pub bytes: &'a [u8],
}

/// Why a verified Player document could not enter a projected resource.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, JsonSchema)]
#[non_exhaustive]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum PlayerDocumentRejection {
    /// The XML parser rejected the verified bytes.
    MalformedXml,
    /// The parsed root did not match the selected resource kind.
    UnexpectedRoot {
        /// Original XML encoding interpreted by the source reader.
        format: &'static str,
        /// Whether source string repair occurred during decoding.
        lossy: bool,
    },
}

/// Verified original bytes for a rejected Player resource.
#[derive(Debug)]
#[non_exhaustive]
pub struct RejectedPlayerDocument<'a> {
    /// Physical identity shared with the rejected resource and other observations.
    pub source: PhysicalSourceRef,
    /// Exact original archive Entry and containing context.
    pub original: &'a EntryOrigin,
    /// Evidence of the observed member payload or of its rejected root.
    pub origin: FieldOrigin,
    /// The observed parser or root rejection.
    pub rejection: PlayerDocumentRejection,
    /// Exact decoded bytes of the physical Archive member.
    pub bytes: &'a [u8],
}

/// Receives CRC-verified Player originals.
pub trait PlayerDocumentSink {
    /// Retains one bound source document and its original bytes.
    ///
    /// # Errors
    /// Returns the caller's failure to retain the document.
    fn document(&mut self, document: PlayerDocument<'_>) -> io::Result<()>;

    /// Retains a verified original whose XML or root was rejected.
    ///
    /// A source can occur both here and in [`Self::document`].
    ///
    /// # Errors
    /// Returns the caller's failure to retain the rejected document.
    fn rejected_document(&mut self, _document: RejectedPlayerDocument<'_>) -> io::Result<()> {
        Ok(())
    }
}

/// Shared preparation of explicit Player bodies and their common authored definitions.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerSourceCapture {
    pub(crate) bodies: Vec<CapturedBody>,
    pub(crate) loadouts: Vec<SourcedValue<Loadout>>,
    pub(crate) motion: Motion,
    pub(crate) controller_topology: Vec<ControllerTopology>,
    pub(crate) actor_rig_candidates: Option<SourcedValue<Vec<RigCandidate>>>,
    pub(crate) adb_declarations: Vec<AdbDeclarations>,
    pub(crate) tag_definitions: TagDefinitions,
    pub(crate) adb_tag_uses: AdbTagUses,
    pub(crate) weapon_motion_sources: WeaponMotionSources,
    pub(crate) weapon_motion_tags: Vec<WeaponMotionTag>,
    pub(crate) source_inventory: SourceInventory,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CapturedBody {
    pub(crate) body_variant: &'static str,
    pub(crate) body: SourcedValue<Body>,
}

pub(super) struct PreparedCapture<'sink> {
    pub(super) bodies: Vec<CapturedBody>,
    pub(super) loadouts: Vec<SourcedValue<Loadout>>,
    pub(super) motion: Motion,
    controller_topology: Vec<ControllerTopology>,
    actor_rig_candidates: Option<SourcedValue<Vec<RigCandidate>>>,
    pub(super) adb_declarations: Vec<AdbDeclarations>,
    pub(super) tag_definitions: TagDefinitions,
    pub(super) documents: XmlDocuments<'sink>,
    pub(super) source_inventory: SourceInventory,
    adb_tag_uses: AdbTagUses,
    weapon_motion_sources: WeaponMotionSources,
    weapon_motion_tags: Vec<WeaponMotionTag>,
}

impl PlayerSourceCapture {
    /// Captures ordered distinct body selections with one shared motion and loadout evaluation.
    ///
    /// # Errors
    /// Returns invalid body selection, cancellation, archive mutation, source or sink errors.
    pub fn read(
        archive: &Archive,
        variants: &[BodyVariant],
        cancel: &CancelToken,
        sink: &mut dyn PlayerDocumentSink,
    ) -> Result<Self, PlayerSourceError> {
        prepare(archive, variants, cancel, XmlDocuments::stream_to(sink))
            .map(PreparedCapture::into_capture)
    }
}

impl PreparedCapture<'_> {
    fn into_capture(self) -> PlayerSourceCapture {
        PlayerSourceCapture {
            bodies: self.bodies,
            loadouts: self.loadouts,
            motion: self.motion,
            controller_topology: self.controller_topology,
            actor_rig_candidates: self.actor_rig_candidates,
            adb_tag_uses: self.adb_tag_uses,
            weapon_motion_sources: self.weapon_motion_sources,
            weapon_motion_tags: self.weapon_motion_tags,
            adb_declarations: self.adb_declarations,
            tag_definitions: self.tag_definitions,
            source_inventory: self.source_inventory,
        }
    }
}

pub(super) fn prepare<'sink>(
    archive: &Archive,
    variants: &[BodyVariant],
    cancel: &CancelToken,
    documents: XmlDocuments<'sink>,
) -> Result<PreparedCapture<'sink>, PlayerSourceError> {
    prepare_observed(archive, variants, cancel, documents, |_, _| {})
}

fn prepare_observed<'sink>(
    archive: &Archive,
    variants: &[BodyVariant],
    cancel: &CancelToken,
    mut documents: XmlDocuments<'sink>,
    observe: impl FnOnce(Option<&DataForgeEntry>, crate::player_source_classes::Classes<'_, '_>),
) -> Result<PreparedCapture<'sink>, PlayerSourceError> {
    validate_body_selection(variants)?;
    let watch = ArchiveWatch::new(archive);
    watch.check()?;
    let sources = SourceUses::with_compact_use_refs();
    let keys = PathKeys::try_new_with(archive, |_| check(cancel)).map_err(index_error)?;
    let paths = keys.index(archive);
    let directory = sources
        .observe_root_directory_paths(archive)?
        .ok_or(PlayerSourceError::RootArchive)?;
    let data = DataForgeEntry::read_until(archive, &|| cancel.is_cancelled()).ok();
    check(cancel)?;
    let opened = data.as_ref().and_then(|data| data.open().ok());
    let forge = opened.as_ref().map(Entities::new);
    let data_scope = data
        .as_ref()
        .map(|data| scope(&sources, data.origin()))
        .transpose()?
        .flatten();
    let classes = forge.as_ref().zip(data_scope.as_ref());
    observe(data.as_ref(), classes);
    let bodies = capture_bodies(&paths, variants, &sources, &directory, classes, cancel)?;
    let loadouts = player_source_loadout::read(
        archive,
        &sources,
        &directory,
        classes,
        &mut documents,
        cancel,
    )?;
    let resources = Resources::new(archive, &sources, cancel);
    let original_motion = OriginalMotion::read(&resources, &directory, classes, &mut documents)?;
    let actor_rig_candidates = documents
        .is_streaming()
        .then(|| read_rig_candidates(classes, &directory, &sources, cancel))
        .transpose()?;
    let (weapon_motion_sources, weapon_motion_tags) = weapon_motion(
        &documents,
        &loadouts,
        classes,
        &sources,
        &original_motion.adb_tag_uses,
        cancel,
    )?;
    watch.check()?;
    Ok(PreparedCapture {
        bodies,
        loadouts,
        motion: original_motion.motion,
        controller_topology: documents.bind_controller_topology(&sources, cancel)?,
        actor_rig_candidates,
        adb_declarations: original_motion.adb_declarations,
        tag_definitions: original_motion.tag_definitions,
        documents,
        source_inventory: sources.freeze()?,
        adb_tag_uses: original_motion.adb_tag_uses,
        weapon_motion_sources,
        weapon_motion_tags,
    })
}

fn capture_bodies(
    paths: &p4kx_extraction::PathIndex<'_>,
    variants: &[BodyVariant],
    sources: &SourceUses,
    directory: &p4kx_extraction::SourceUse,
    classes: crate::player_source_classes::Classes<'_, '_>,
    cancel: &CancelToken,
) -> Result<Vec<CapturedBody>, PlayerSourceError> {
    variants
        .iter()
        .map(|&variant| {
            Ok(CapturedBody {
                body_variant: variant.code(),
                body: player_source_body::read(
                    paths, variant, sources, directory, classes, cancel,
                )?,
            })
        })
        .collect()
}

struct OriginalMotion {
    motion: Motion,
    adb_declarations: Vec<AdbDeclarations>,
    tag_definitions: TagDefinitions,
    adb_tag_uses: AdbTagUses,
}

impl OriginalMotion {
    fn read(
        resources: &Resources<'_>,
        directory: &p4kx_extraction::SourceUse,
        classes: crate::player_source_classes::Classes<'_, '_>,
        documents: &mut XmlDocuments<'_>,
    ) -> Result<Self, PlayerSourceError> {
        let motion = Motion::read(resources, directory, classes, documents)?;
        let adb_declarations =
            documents.bind_adb_declarations(resources.sources, resources.cancel)?;
        let tag_definitions = documents.bind_tag_definitions(resources)?;
        let adb_tag_uses = AdbTagUses::read(
            &adb_declarations,
            &tag_definitions,
            resources.sources,
            resources.cancel,
        )?;
        Ok(Self {
            motion,
            adb_declarations,
            tag_definitions,
            adb_tag_uses,
        })
    }
}

fn weapon_motion(
    documents: &XmlDocuments<'_>,
    loadouts: &[SourcedValue<Loadout>],
    classes: crate::player_source_classes::Classes<'_, '_>,
    sources: &SourceUses,
    adb_tag_uses: &AdbTagUses,
    cancel: &CancelToken,
) -> Result<(WeaponMotionSources, Vec<WeaponMotionTag>), PlayerSourceError> {
    if !documents.is_streaming() {
        return Ok((WeaponMotionSources::default(), Vec::new()));
    }
    let weapons = player_weapon_motion_sources::read(loadouts, classes, sources, cancel)?;
    let tags = adb_tag_uses.weapon_tags(&weapons.weapons, sources, cancel)?;
    Ok((weapons, tags))
}

fn validate_body_selection(variants: &[BodyVariant]) -> Result<(), PlayerSourceError> {
    if variants.is_empty()
        || variants.iter().enumerate().any(|(index, variant)| {
            variants
                .iter()
                .take(index)
                .any(|previous| previous == variant)
        })
    {
        return Err(PlayerSourceError::BodySelection);
    }
    Ok(())
}

fn scope(
    sources: &SourceUses,
    origin: &DataForgeOrigin,
) -> Result<Option<SourceScope>, SourceEvidenceError> {
    match origin {
        DataForgeOrigin::Archive(original) => sources
            .observe(SourceInput::Archive(original), InputScope::Payload)
            .map(Some),
        _ => Ok(None),
    }
}

#[cfg(test)]
#[path = "player_adb_installed_witness.rs"]
mod installed_witness;

#[cfg(test)]
#[path = "player_bundle_installed_witness.rs"]
mod bundle_witness;
