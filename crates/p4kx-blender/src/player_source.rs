// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{BodyVariant, SocCaptureError};
use p4kx_extraction::{
    Archive, ArchiveChanged, CancelToken, PathIndexBuildError, PathIndexError, SourceEvidenceError,
    SourceInventory, SourcedValue,
};
use schemars::JsonSchema;
use serde::Serialize;
use thiserror::Error;

use crate::player_source_body::Body;
use crate::player_source_loadout::Loadout;
use crate::player_source_motion::Motion;
use crate::soc_scene::SocSceneError;
use crate::soc_scene_xml::{XmlDocument, XmlDocuments};

#[path = "player_source_capture.rs"]
mod capture;

use capture::prepare;
pub use capture::{
    PlayerDocument, PlayerDocumentRejection, PlayerDocumentSink, PlayerSourceCapture,
    RejectedPlayerDocument,
};

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
        let mut capture = prepare(archive, &[variant], cancel, XmlDocuments::default())?;
        let body = capture.bodies.pop().ok_or(PlayerSourceError::BodyVariant)?;
        Ok(Self {
            format: "p4kx-player-source",
            schema_version: 1,
            body_variant: variant.code(),
            body: body.body,
            loadouts: capture.loadouts,
            motion: capture.motion,
            documents: capture.documents.finish(),
            source_inventory: capture.source_inventory,
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
    /// The caller selected no body or repeated a body in the same capture.
    #[error("select at least one distinct player body")]
    BodySelection,
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
