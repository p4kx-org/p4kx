// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

#[cfg(windows)]
use crate::portable_name::fold;
use crate::{ConvertedSoundArtifact, RawEntryArtifact};

#[derive(Default)]
pub(crate) struct ExtractionArtifacts {
    files: Mutex<HashMap<PathBuf, Artifact>>,
}

enum Artifact {
    Raw(RawEntryArtifact),
    Sound(ConvertedSoundArtifact),
}

#[derive(Default)]
pub(crate) struct ArtifactReports {
    pub(crate) raw: Vec<RawEntryArtifact>,
    pub(crate) sound: Vec<ConvertedSoundArtifact>,
}

impl ExtractionArtifacts {
    pub(crate) fn insert_raw(&self, output: &Path, artifact: RawEntryArtifact) {
        self.insert(output, Artifact::Raw(artifact));
    }

    pub(crate) fn insert_sound(&self, output: &Path, artifact: ConvertedSoundArtifact) {
        self.insert(output, Artifact::Sound(artifact));
    }

    fn insert(&self, output: &Path, artifact: Artifact) {
        self.files
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(key(output), artifact);
    }

    pub(crate) fn invalidate(&self, output: &Path) {
        self.files
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&key(output));
    }

    pub(crate) fn into_sorted(self) -> ArtifactReports {
        let mut reports = ArtifactReports::default();
        for entry in self
            .files
            .into_inner()
            .unwrap_or_else(PoisonError::into_inner)
            .into_values()
        {
            match entry {
                Artifact::Raw(artifact) => reports.raw.push(artifact),
                Artifact::Sound(artifact) => reports.sound.push(artifact),
            }
        }
        reports.raw.sort_unstable_by(|a, b| a.path.cmp(&b.path));
        reports.sound.sort_unstable_by(|a, b| a.path.cmp(&b.path));
        reports
    }
}

#[cfg(windows)]
fn key(output: &Path) -> PathBuf {
    fold(&output.to_string_lossy()).collect::<String>().into()
}

#[cfg(not(windows))]
fn key(output: &Path) -> PathBuf {
    output.to_path_buf()
}
