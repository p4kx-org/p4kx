// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{BodyVariant, SocCaptureError};
use p4kx_extraction::{
    Archive, ArchiveChanged, ArchiveWatch, CancelToken, DataForgeEntry, DataForgeOrigin,
    InputScope, PathIndexBuildError, PathIndexError, PathKeys, SourceEvidenceError, SourceInput,
    SourceInventory, SourceScope, SourceUses, SourcedValue,
};
use schemars::JsonSchema;
use serde::Serialize;
use thiserror::Error;

use crate::player_source_body::{self, Body};
use crate::player_source_loadout::{self, Loadout};
use crate::player_source_motion::Motion;
use crate::soc_scene::SocSceneError;
use crate::soc_scene_class_resources::Resources;
use crate::soc_scene_xml::{XmlDocument, XmlDocuments};

/// Original character defaults and player loadout declarations.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlayerSource {
    #[schemars(extend("const" = "p4kx-player-source"))]
    format: &'static str,
    #[schemars(extend("const" = 1))]
    schema_version: u8,
    body_variant: &'static str,
    body: SourcedValue<Body>,
    loadouts: Vec<SourcedValue<Loadout>>,
    motion: Motion,
    documents: Vec<XmlDocument>,
    source_inventory: SourceInventory,
}

impl PlayerSource {
    /// Reads an explicit body variant and its scoped original player loadout declarations.
    ///
    /// # Errors
    /// Returns cancellation, archive mutation, selection or source-evidence errors.
    pub fn read(
        archive: &Archive,
        variant: BodyVariant,
        cancel: &CancelToken,
    ) -> Result<Self, PlayerSourceError> {
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
        let forge = opened.as_ref().map(p4kx_assembly::Entities::new);
        let data_scope = data
            .as_ref()
            .map(|data| scope(&sources, data.origin()))
            .transpose()?
            .flatten();
        let classes = forge.as_ref().zip(data_scope.as_ref());
        let body =
            player_source_body::read(&paths, variant, &sources, &directory, classes, cancel)?;
        let mut documents = XmlDocuments::default();
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
        Ok(Self {
            format: "p4kx-player-source",
            schema_version: 1,
            body_variant: variant.code(),
            body,
            loadouts,
            motion,
            documents: documents.finish(),
            source_inventory: sources.freeze()?,
        })
    }
}

/// Why an authored player source export could not finish.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum PlayerSourceError {
    /// The selection requires the original root archive.
    #[error("player source export requires a root Archive")]
    RootArchive,
    /// The selected body variant has no verified archive selector.
    #[error("unknown player body variant")]
    BodyVariant,
    /// Source capture was cancelled.
    #[error("player source export cancelled")]
    Cancelled,
    /// The original archive changed.
    #[error(transparent)]
    ArchiveChanged(#[from] ArchiveChanged),
    /// The physical path index could not be prepared.
    #[error(transparent)]
    Index(#[from] PathIndexError),
    /// Source evidence could not be prepared.
    #[error(transparent)]
    Evidence(#[from] SourceEvidenceError),
    /// Original XML evidence could not be projected.
    #[error(transparent)]
    Xml(#[from] SocSceneError),
    /// An original archive source could not be captured.
    #[error(transparent)]
    Capture(#[from] SocCaptureError),
    /// The selected member no longer has an exact physical binding.
    #[error("player source member has no physical archive binding")]
    Binding,
}

/// Returns the generated authored player source schema.
#[must_use]
pub fn player_source_schema() -> serde_json::Value {
    schemars::schema_for!(PlayerSource).to_value()
}

pub(crate) fn check(cancel: &CancelToken) -> Result<(), PlayerSourceError> {
    if cancel.is_cancelled() {
        Err(PlayerSourceError::Cancelled)
    } else {
        Ok(())
    }
}

pub(crate) fn index_error(error: PathIndexBuildError<PlayerSourceError>) -> PlayerSourceError {
    match error.into_cause() {
        Ok(error) => error,
        Err(error) => error.into(),
    }
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
