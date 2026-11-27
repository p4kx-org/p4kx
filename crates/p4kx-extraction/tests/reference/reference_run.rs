// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use p4kx_extraction::{Archive, ArchiveWatch};

use super::reference_artifacts;
use super::reference_digest;
use super::reference_error::{Code, ReferenceError, Result};
use super::reference_layout;
use super::reference_load;
use super::reference_manifest::{Artifact, Manifest, SelectionMode, Tool};
use super::reference_paths;
use super::reference_source::{self, MeasuredSource};
use super::tally::Tally;

pub(crate) struct Dataset {
    pub(crate) root: PathBuf,
    pub(crate) manifest: Manifest,
    pub(crate) source: MeasuredSource,
    manifest_fingerprint: super::reference_manifest::Fingerprint,
    artifacts: BTreeMap<String, usize>,
}

impl Dataset {
    pub(crate) fn load(root: &Path, archive: &Archive, tool: Tool) -> Result<Self> {
        let root = reference_paths::root(root)?;
        let loaded = reference_load::read(&root)?;
        let manifest = loaded.manifest;
        reference_layout::shape(&manifest, tool, true)?;
        reference_artifacts::verify(&root, &manifest)?;
        let source = reference_source::measure(archive, tool)?;
        source.validate(&manifest)?;
        source.verify_source_copies(archive, &root, &manifest)?;
        let artifacts = manifest
            .artifacts
            .iter()
            .enumerate()
            .map(|(index, artifact)| Ok((reference_paths::key(&artifact.path)?, index)))
            .collect::<Result<_>>()?;
        Ok(Self {
            root,
            manifest,
            source,
            manifest_fingerprint: loaded.fingerprint,
            artifacts,
        })
    }

    pub(crate) fn artifact(&self, path: &str) -> Result<&Artifact> {
        self.artifacts
            .get(&reference_paths::key(path)?)
            .and_then(|index| self.manifest.artifacts.get(*index))
            .ok_or_else(|| {
                ReferenceError::new(
                    Code::Coverage,
                    "comparison requested an undeclared artifact",
                )
            })
    }

    pub(crate) fn read(&self, path: &str) -> Result<Vec<u8>> {
        reference_artifacts::read(&self.root, self.artifact(path)?)
    }

    pub(crate) fn ensure_unchanged(&self, archive: &Archive) -> Result<()> {
        ArchiveWatch::new(archive)
            .check()
            .map_err(|error| ReferenceError::new(Code::Changed, error.to_string()))?;
        let path = self.root.join(super::reference_manifest::MANIFEST_NAME);
        if reference_digest::file(&path, super::reference_manifest::MAX_MANIFEST_BYTES)?
            != self.manifest_fingerprint
        {
            return Err(ReferenceError::new(
                Code::Changed,
                "reference manifest changed during comparison",
            ));
        }
        Ok(())
    }

    pub(crate) fn finish(&self, archive: &Archive, tally: &Tally, name: &str) -> Result<()> {
        self.ensure_unchanged(archive)?;
        let standalone = self
            .manifest
            .expected
            .iter()
            .flat_map(|source| &source.targets)
            .filter(|target| target.subresource.is_none())
            .count();
        let embedded = self
            .manifest
            .expected
            .iter()
            .flat_map(|source| &source.targets)
            .filter(|target| target.subresource.is_some())
            .count();
        if self.manifest.selection.mode == SelectionMode::MainOnly {
            super::report(
                "main-only disposition: unverified producer claim; declared GUID/DCB subset and outputs are locally checked",
            );
        }
        super::report(&format!(
            "{name} declared scope {:?}: {} selected logical sources of {}, {} standalone and {} embedded locations, {} verified artifacts",
            self.manifest.selection.mode,
            self.manifest.expected.len(),
            self.source.universe_len(),
            standalone,
            embedded,
            self.manifest.artifacts.len()
        ));
        if self.manifest.expected.is_empty() {
            return Err(ReferenceError::new(
                Code::Coverage,
                "explicit empty selection: no independent comparisons",
            ));
        }
        if tally.compared != self.manifest.expected.len() as u64 {
            return Err(ReferenceError::new(
                Code::Coverage,
                "compared logical identities do not match selection",
            ));
        }
        tally
            .finish(name)
            .map_err(|error| ReferenceError::new(Code::Coverage, error))
    }
}

pub(crate) fn optional(
    root: Option<PathBuf>,
    name: &str,
    run: impl FnOnce(&Path) -> super::TestResult,
) -> super::TestResult {
    let Some(root) = root else {
        super::report(&format!("skipped: {name} is not configured"));
        return Ok(());
    };
    run(&root)
}
