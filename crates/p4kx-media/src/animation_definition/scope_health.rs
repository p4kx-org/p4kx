// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{RigDefinition, RigFile, RigFileRead, RigParameter};
use p4kx_extraction::model::SkeletonSource;

use crate::MediaError;

pub(super) fn verify(
    definition: Option<&RigFile<RigDefinition>>,
    skeleton: Option<&RigFile<Option<SkeletonSource>>>,
    parameters: Option<&RigFile<Vec<RigParameter>>>,
) -> Result<(), MediaError> {
    if let Some(definition) = definition {
        verify_definition(definition, skeleton)?;
    }
    if let Some(parameters) = parameters
        && !matches!(parameters.read.as_ref(), RigFileRead::Missing)
    {
        verify_read(parameters, "character parameters")?;
    }
    Ok(())
}

fn verify_definition(
    definition: &RigFile<RigDefinition>,
    skeleton: Option<&RigFile<Option<SkeletonSource>>>,
) -> Result<(), MediaError> {
    verify_read(definition, "CDF")?;
    if matches!(definition.read.as_ref(), RigFileRead::NotSelected) {
        return Ok(());
    }
    let Some(skeleton) = skeleton else {
        return Err(blocked(definition, "CDF", "no usable Model File"));
    };
    match skeleton.read.as_ref() {
        RigFileRead::Missing => Err(blocked(skeleton, "CDF model", "missing Entry")),
        RigFileRead::Ambiguous => Err(blocked(skeleton, "CDF model", "ambiguous Entry")),
        RigFileRead::Unreadable { .. } => Err(blocked(skeleton, "CDF model", "unproved Entry")),
        _ => Ok(()),
    }
}

fn verify_read<T>(file: &RigFile<T>, role: &str) -> Result<(), MediaError> {
    let reason = match file.read.as_ref() {
        RigFileRead::NotSelected | RigFileRead::Read { value: Ok(_), .. } => return Ok(()),
        RigFileRead::Missing => "missing Entry",
        RigFileRead::Ambiguous => "ambiguous Entry",
        RigFileRead::KeyRequired => "Archive key required",
        RigFileRead::WrongKey => "Archive key rejected",
        RigFileRead::Unreadable { .. } => "unreadable Entry",
        RigFileRead::Read { value: Err(_), .. } => "invalid source document",
        _ => "unsupported read outcome",
    };
    Err(blocked(file, role, reason))
}

fn blocked<T>(file: &RigFile<T>, role: &str, reason: &str) -> MediaError {
    MediaError::Contract(format!(
        "incomplete animation definition scope: {role} {}: {reason}",
        file.path
    ))
}
