// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_archive::BuildManifestOrigin;

use crate::ObservedFileOrigin;
use crate::source_value::SourceValue;

pub(crate) enum FileInput {
    Observed(ObservedFileOrigin),
    BuildManifest(BuildManifestOrigin),
}

#[derive(Clone, Copy)]
pub(crate) enum FileInputRef<'a> {
    Observed(&'a ObservedFileOrigin),
    BuildManifest(&'a BuildManifestOrigin),
}

impl FileInputRef<'_> {
    pub(crate) fn is_same_input(self, other: &FileInput) -> bool {
        match (self, other) {
            (Self::Observed(left), FileInput::Observed(right)) => left.is_same_input(right),
            (Self::BuildManifest(left), FileInput::BuildManifest(right)) => {
                left.is_same_input(right)
            }
            _ => false,
        }
    }

    pub(crate) fn into_owned(self) -> FileInput {
        match self {
            Self::Observed(origin) => FileInput::Observed(origin.clone()),
            Self::BuildManifest(origin) => FileInput::BuildManifest(origin.clone()),
        }
    }

    pub(crate) fn name(self) -> SourceValue<String> {
        match self {
            Self::Observed(origin) => origin
                .path()
                .file_name()
                .map_or(SourceValue::Absent, |name| {
                    SourceValue::Present(name.to_string_lossy().into_owned())
                }),
            Self::BuildManifest(origin) => SourceValue::Present(origin.name().to_owned()),
        }
    }

    pub(crate) fn byte_length(self) -> u64 {
        match self {
            Self::Observed(origin) => origin.stamp().size(),
            Self::BuildManifest(origin) => origin.byte_length(),
        }
    }
}
