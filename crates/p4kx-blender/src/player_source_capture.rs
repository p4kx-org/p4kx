// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use p4kx_assembly::{BodyVariant, Entities};
use p4kx_extraction::{
    Archive, ArchiveWatch, CancelToken, DataForgeEntry, DataForgeOrigin, EntryOrigin, FieldOrigin,
    InputScope, PathKeys, PhysicalSourceRef, SourceEvidenceError, SourceInput, SourceInventory,
    SourceScope, SourceUses, SourcedValue,
};
use serde::Serialize;

use crate::player_source_body::{self, Body};
use crate::player_source_loadout::{self, Loadout};
use crate::player_source_motion::Motion;
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

/// Receives the verified original XML before its parse is released.
pub trait PlayerDocumentSink {
    /// Retains one bound source document and its original bytes.
    ///
    /// # Errors
    /// Returns the caller's failure to retain the document.
    fn document(&mut self, document: PlayerDocument<'_>) -> io::Result<()>;
}

/// Shared preparation of explicit Player bodies and their common authored definitions.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerSourceCapture {
    bodies: Vec<CapturedBody>,
    loadouts: Vec<SourcedValue<Loadout>>,
    motion: Motion,
    source_inventory: SourceInventory,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CapturedBody {
    body_variant: &'static str,
    pub(super) body: SourcedValue<Body>,
}

pub(super) struct PreparedCapture<'sink> {
    pub(super) bodies: Vec<CapturedBody>,
    pub(super) loadouts: Vec<SourcedValue<Loadout>>,
    pub(super) motion: Motion,
    pub(super) documents: XmlDocuments<'sink>,
    pub(super) source_inventory: SourceInventory,
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
        let prepared = prepare(archive, variants, cancel, XmlDocuments::stream_to(sink))?;
        Ok(Self {
            bodies: prepared.bodies,
            loadouts: prepared.loadouts,
            motion: prepared.motion,
            source_inventory: prepared.source_inventory,
        })
    }
}

pub(super) fn prepare<'sink>(
    archive: &Archive,
    variants: &[BodyVariant],
    cancel: &CancelToken,
    mut documents: XmlDocuments<'sink>,
) -> Result<PreparedCapture<'sink>, PlayerSourceError> {
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
    let mut bodies = Vec::with_capacity(variants.len());
    for &variant in variants {
        let body =
            player_source_body::read(&paths, variant, &sources, &directory, classes, cancel)?;
        bodies.push(CapturedBody {
            body_variant: variant.code(),
            body,
        });
    }
    let loadouts = player_source_loadout::read(
        archive,
        &sources,
        &directory,
        classes,
        &mut documents,
        cancel,
    )?;
    let motion = Motion::read(
        &Resources::new(archive, &sources, cancel),
        &directory,
        classes,
        &mut documents,
    )?;
    watch.check()?;
    Ok(PreparedCapture {
        bodies,
        loadouts,
        motion,
        documents,
        source_inventory: sources.freeze()?,
    })
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
