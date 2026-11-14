// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_character::{Dna, DnaRig, FaceRig, RigLogic};

use super::RigArchiveRead;
use super::facial::{FaceLink, FaceSource};
use super::shape::rig_bytes;
use crate::ModelProvider;

pub(super) struct Loaded {
    pub(super) face: FaceRig,
    pub(super) origin: Option<RigArchiveRead>,
}

pub(super) fn load(link: &FaceLink, models: &dyn ModelProvider) -> Result<Loaded, String> {
    match &link.source {
        FaceSource::Dna { rig, dna } => dna_face(rig, dna, &link.logic, models),
        FaceSource::Own => FaceRig::from_logic(&link.logic)
            .map(|face| Loaded { face, origin: None })
            .map_err(|error| error.to_string()),
    }
}

fn dna_face(
    rig_path: &str,
    dna: &Dna,
    logic: &RigLogic,
    models: &dyn ModelProvider,
) -> Result<Loaded, String> {
    let (bytes, origin) = rig_bytes(rig_path, models)?;
    let rig = DnaRig::read(&bytes).map_err(|error| format!("{rig_path}: {error}"))?;
    let matrix = rig
        .matrix(dna)
        .map_err(|error| format!("{rig_path}: {error}"))?;
    let face =
        FaceRig::new(&rig, &matrix, logic).map_err(|error| format!("{rig_path}: {error}"))?;
    Ok(Loaded {
        face,
        origin: Some(origin),
    })
}
