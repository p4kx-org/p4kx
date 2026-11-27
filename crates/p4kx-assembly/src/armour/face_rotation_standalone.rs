// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

#[path = "face_rotation_source.rs"]
mod source;

use p4kx_chunks::ChunkFile;
use p4kx_model::{Animation, AnimationSource};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::super::super::facial::FaceSource;
use super::prepared::Prepared;
use super::{Armour, FaceRotationComparison, ModelProvider};

use self::source::ControlEvidence;

/// A standalone four-convention comparison with source-owned control-default evidence.
#[derive(Debug)]
pub struct StandaloneFaceRotationComparison {
    comparison: FaceRotationComparison,
    file_name: String,
    digest: String,
    selected: Value,
    controls: Vec<ControlEvidence>,
}

impl StandaloneFaceRotationComparison {
    /// Returns the common comparison without asserting runtime control initialization.
    #[must_use]
    pub const fn comparison(&self) -> &FaceRotationComparison {
        &self.comparison
    }

    /// Returns the modeled-control profile and original occurrences, followed by the comparison.
    pub fn verification_records(&self) -> impl Iterator<Item = Value> + '_ {
        std::iter::once_with(move || {
            json!({
                "record": "standaloneFaceProfile",
                "profileVersion": 1,
                "initialization": "zeroUnkeyedNonconstantControls",
                "constantControlSlot": 0,
                "constantControlValue": 1,
                "source": {"kind": "cafBytes", "name": self.file_name,
                    "sha256": self.digest, "archiveBinding": "notEstablished"},
                "selected": self.selected,
                "sourceControllerOccurrences": self.controls.len(),
                "runtimeInitialization": "notEstablished",
                "selectedConvention": null,
                "referenceComparison": "notPerformed",
            })
        })
        .chain(self.controls.iter().map(ControlEvidence::record))
        .chain(self.comparison.verification_records())
    }
}

impl Armour {
    /// Compares a source-owned CAF with unkeyed controls initialized by the standalone profile.
    ///
    /// # Errors
    /// Returns canonical CAF errors or an error for an unsupported control, ambiguous target,
    /// non-neutral scalar-less payload, invalid frame interval or unavailable rig.
    pub fn verify_standalone_face_rotations_from_caf(
        &self,
        file_name: &str,
        bytes: &[u8],
        models: &dyn ModelProvider,
        frames: &[u16],
    ) -> Result<StandaloneFaceRotationComparison, String> {
        let file = ChunkFile::parse(bytes).map_err(|error| error.to_string())?;
        let positioned = Animation::read_caf_with_positions(file_name, &file)
            .map_err(|error| error.to_string())?;
        let original =
            AnimationSource::read_caf(file_name, &file).map_err(|error| error.to_string())?;
        let prepared = Prepared::read(self, positioned.animation(), models, frames)?;
        let controls = source::controls(&original, &positioned, prepared.rig.face.logic())?;
        let rig = match &prepared.link.source {
            FaceSource::Dna { rig, .. } => json!({"kind": "dna", "path": rig}),
            FaceSource::Own => json!({"kind": "own", "path": null}),
        };
        let selected = json!({
            "part": {"index": prepared.part, "name": prepared.selected.name(),
                "file": prepared.selected.file()},
            "rig": rig,
            "rigControls": prepared.rig.face.logic().controls(),
            "rigBlendshapes": prepared.rig.face.logic().blendshapes(),
        });
        let mut animation = positioned.into_animation();
        let mut neutral = controls
            .iter()
            .filter_map(ControlEvidence::projected_track)
            .peekable();
        let mut position = 0;
        animation.tracks.retain(|_| {
            let discard = neutral.peek().copied() == Some(position);
            if discard {
                neutral.next();
            }
            position += 1;
            !discard
        });
        let comparison = super::complete(self, &animation, prepared, frames)?;
        let hash = Sha256::digest(bytes);
        let mut digest = String::with_capacity(hash.len() * 2);
        for byte in hash {
            use std::fmt::Write as _;
            write!(digest, "{byte:02x}").map_err(|error| error.to_string())?;
        }
        Ok(StandaloneFaceRotationComparison {
            comparison,
            file_name: file_name.to_owned(),
            digest,
            selected,
            controls,
        })
    }
}
