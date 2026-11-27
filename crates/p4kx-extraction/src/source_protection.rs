// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;
use std::sync::atomic::AtomicBool;

use crate::{ArchiveWriteProtection, ConversionError, WriteProtectionError};

/// Retained origin evidence that does not turn an uncertain write into a read failure.
#[derive(Debug, Clone)]
pub(crate) struct SourceProtection {
    archive: Option<Result<ArchiveWriteProtection, WriteProtectionError>>,
}

impl SourceProtection {
    pub(crate) fn new(archive: Option<&Path>) -> Self {
        Self {
            archive: archive.map(|path| ArchiveWriteProtection::new(path, &AtomicBool::new(false))),
        }
    }

    pub(crate) fn context(&self) -> Result<Option<&ArchiveWriteProtection>, ConversionError> {
        self.archive
            .as_ref()
            .map(|result| result.as_ref().map_err(|error| error.clone().into()))
            .transpose()
    }
}
