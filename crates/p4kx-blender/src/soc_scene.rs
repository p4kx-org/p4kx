// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{SocCaptureError, SocContainerCapture};
use p4kx_extraction::{CancelToken, SourceEvidenceError, SourceInventory, SourceUses};
use schemars::JsonSchema;
use serde::Serialize;
use thiserror::Error;

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
        let sources = SourceUses::with_compact_use_refs();
        let mut documents = XmlDocuments::default();
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
                    &mut documents,
                    cancel,
                )
            })
            .collect::<Result<_, _>>()?;
        Ok(Self {
            format: "p4kx-soc-scene",
            schema_version: SOC_SCENE_VERSION,
            source_inventory: sources.freeze()?,
            documents: documents.finish(),
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
    /// Source projection was interrupted.
    #[error("SOC scene projection cancelled")]
    Cancelled,
}

/// Returns the generated schema for the standalone SOC scene document.
#[must_use]
pub fn soc_scene_schema() -> serde_json::Value {
    schemars::schema_for!(SocScene).to_value()
}
