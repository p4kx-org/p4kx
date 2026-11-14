// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{SocCaptureError, SocContainerCapture};
use p4kx_extraction::{
    Archive, ArchiveWatch, CancelToken, SourceEvidenceError, SourceInventory, SourceUses,
};
use schemars::JsonSchema;
use serde::Serialize;
use thiserror::Error;

use crate::soc_scene_classes::{Classes, SocClass};
use crate::soc_scene_included::{IncludedSource, IncludedSources};
use crate::soc_scene_light_classes::DefaultClass;
pub use crate::soc_scene_origin::SocXmlPosition;
use crate::soc_scene_xml::{XmlDocument, XmlDocuments};

/// Independently versioned contract of the authored SOC scene document.
pub const SOC_SCENE_VERSION: u8 = 1;

/// Versioned original SOC scene data with per-value physical evidence.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SocScene {
    #[schemars(extend("const" = "p4kx-soc-scene"))]
    format: &'static str,
    #[schemars(extend("const" = SOC_SCENE_VERSION))]
    schema_version: u8,
    source_inventory: SourceInventory,
    documents: Vec<XmlDocument>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    included_objects: Vec<IncludedSource>,
    classes: Vec<SocClass>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    light_classes: Vec<DefaultClass>,
    occurrences: Vec<crate::soc_scene_occurrence::Occurrence>,
}

impl SocScene {
    /// Projects original SOC data without reading models or deriving gameplay behavior.
    ///
    /// # Errors
    /// Returns source-evidence or coordinate errors that prevent accurate output.
    pub fn from_capture(capture: &SocContainerCapture) -> Result<Self, SocSceneError> {
        Self::from_capture_with_cancel(capture, &CancelToken::new())
    }

    /// Projects the source while observing caller cancellation between source documents.
    ///
    /// # Errors
    /// Returns cancellation, source-evidence or coordinate errors.
    pub fn from_capture_with_cancel(
        capture: &SocContainerCapture,
        cancel: &CancelToken,
    ) -> Result<Self, SocSceneError> {
        Self::project(capture, None, cancel)
    }

    /// Resolves the authored entity class links from the original root Archive.
    ///
    /// # Errors
    /// Returns cancellation, source-evidence or coordinate errors.
    pub fn from_archive_capture(
        capture: &SocContainerCapture,
        archive: &Archive,
        cancel: &CancelToken,
    ) -> Result<Self, SocSceneError> {
        let original = capture
            .occurrences()
            .first()
            .ok_or(SocSceneError::Position)?;
        if archive.root_directory_entry_count().is_none()
            || !original
                .origin
                .input_origin()
                .is_same_input(archive.input_origin())
        {
            return Err(SocCaptureError::Root.into());
        }
        let watch = ArchiveWatch::new(archive);
        watch.check().map_err(SocCaptureError::Changed)?;
        let scene = Self::project(capture, Some(archive), cancel)?;
        watch.check().map_err(SocCaptureError::Changed)?;
        Ok(scene)
    }

    fn project(
        capture: &SocContainerCapture,
        archive: Option<&Archive>,
        cancel: &CancelToken,
    ) -> Result<Self, SocSceneError> {
        let sources = SourceUses::with_compact_use_refs();
        let classes = archive
            .map(|archive| Classes::read(capture, archive, &sources, cancel))
            .transpose()?
            .unwrap_or_default();
        let mut documents = XmlDocuments::default();
        let mut included = IncludedSources::default();
        let occurrences = capture
            .occurrences()
            .iter()
            .map(|original| {
                if cancel.is_cancelled() {
                    return Err(SocSceneError::Cancelled);
                }
                crate::soc_scene_occurrence::Occurrence::read(
                    original,
                    &sources,
                    (&mut documents, &mut included),
                    &classes,
                    cancel,
                )
            })
            .collect::<Result<_, _>>()?;
        let (classes, light_classes) = classes.finish();
        Ok(Self {
            format: "p4kx-soc-scene",
            schema_version: SOC_SCENE_VERSION,
            source_inventory: sources.freeze()?,
            documents: documents.finish(),
            included_objects: included.finish(),
            classes,
            light_classes,
            occurrences,
        })
    }
}

/// A failure that prevents truthful SOC scene output.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum SocSceneError {
    /// The original archive capture was interrupted.
    #[error(transparent)]
    Capture(#[from] SocCaptureError),
    /// Source evidence could not be prepared.
    #[error(transparent)]
    Evidence(#[from] SourceEvidenceError),
    /// The original XML coordinate cannot be represented.
    #[error("cannot represent an original SOC XML coordinate")]
    Position,
    /// A verified XML input could not be retained by the output sink.
    #[error("cannot retain a verified XML source")]
    DocumentSink(#[source] std::io::Error),
    /// Source projection was interrupted.
    #[error("SOC scene projection cancelled")]
    Cancelled,
    /// Storage for original source values could not be allocated.
    #[error(transparent)]
    Allocation(#[from] std::collections::TryReserveError),
}

/// Returns the generated schema for the standalone SOC scene document.
#[must_use]
pub fn soc_scene_schema() -> serde_json::Value {
    schemars::schema_for!(SocScene).to_value()
}
