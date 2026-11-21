// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};

use p4kx_extraction::{OriginArtifactDigest, OriginDigestWriter};
use p4kx_guard::{OwnedDirectoryPermit, OwnedFileToken};
use tempfile::Builder;

use crate::player_bundle_wire::{self as wire, Manifest};
use crate::{PackageError, PackageResult, PackageWriteContext, error, stage};

pub(super) struct Output<'a> {
    root: OwnedDirectoryPermit,
    context: PackageWriteContext<'a>,
    files: BTreeMap<PathBuf, OwnedFileToken>,
    published: bool,
    originals: Option<OwnedDirectoryPermit>,
}

impl<'a> Output<'a> {
    pub(super) fn create(path: &Path, context: PackageWriteContext<'a>) -> PackageResult<Self> {
        let path = std::path::absolute(path).map_err(|e| PackageError::write(path, e))?;
        let root = context.create_directory(&path)?;
        let mut output = Self {
            root,
            context,
            files: BTreeMap::new(),
            published: false,
            originals: None,
        };
        match context.create_directory(&path.join(wire::ORIGINALS)) {
            Ok(originals) => {
                output.originals = Some(originals);
                Ok(output)
            }
            Err(cause) => output.finish(Err(cause)),
        }
    }

    pub(super) fn path(&self, file: &str) -> PathBuf {
        self.root.path().join(file)
    }

    pub(super) const fn write_context(&self) -> PackageWriteContext<'a> {
        self.context
    }

    pub(super) fn check(&self) -> PackageResult<()> {
        self.context.check()?;
        self.root
            .ensure_namespace_writable(self.root.path(), self.context.cancel_flag())?;
        if let Some(originals) = &self.originals {
            originals.ensure_namespace_writable(originals.path(), self.context.cancel_flag())?;
        }
        Ok(())
    }

    pub(super) fn file(
        &mut self,
        name: &str,
    ) -> PackageResult<OriginDigestWriter<BufWriter<File>>> {
        self.check()?;
        let path = self.path(name);
        let (file, ownership) = self.context.create_file(&path)?.into_parts();
        self.files.insert(path, ownership);
        Ok(OriginDigestWriter::new(BufWriter::new(file)))
    }

    pub(super) fn complete_file(
        &self,
        path: &Path,
        writer: OriginDigestWriter<BufWriter<File>>,
    ) -> PackageResult<OriginArtifactDigest> {
        self.check()?;
        let (digest, sink) = writer
            .finish_with_sink()
            .map_err(|e| PackageError::write(path, e))?;
        self.files
            .get(path)
            .ok_or_else(|| {
                PackageError::write(path, io::Error::other("unexpected completed artifact"))
            })?
            .ensure_writable(path, self.context.cancel_flag())?;
        sink.get_ref()
            .sync_all()
            .map_err(|e| PackageError::write(path, e))?;
        Ok(digest)
    }

    pub(super) fn publish(&mut self, manifest: &Manifest<'_>) -> PackageResult<()> {
        self.check()?;
        let destination = self.path(wire::MANIFEST);
        let directory = self.root.path();
        let temp = Builder::new()
            .prefix(".player-manifest-")
            .disable_cleanup(true)
            .make_in(directory, |path| {
                self.context.create_file(path).map_err(io::Error::other)
            })
            .map_err(|e| PackageError::write(&destination, e))?;
        let (created, temporary) = temp.into_parts();
        let (file, mut ownership) = created.into_parts();
        let path = temporary.to_path_buf();
        let result = write_manifest(&path, file, manifest)
            .and_then(|()| self.sync_before_manifest(&path, &ownership));
        if let Err(cause) = result {
            self.files.insert(path, ownership);
            return Err(cause);
        }
        if let Err(error) = temporary.persist_noclobber(&destination) {
            self.files.insert(path, ownership);
            return Err(PackageError::write(&destination, error.error));
        }
        self.published = true;
        ownership.rebind_after_rename(&destination, self.context.cancel_flag())?;
        stage::sync_folder(self.root.path()).map_err(|e| PackageError::write(&destination, e))
    }

    fn sync_before_manifest(&self, path: &Path, ownership: &OwnedFileToken) -> PackageResult<()> {
        let directory = self.root.path();
        stage::sync_folder(&self.path(wire::ORIGINALS))
            .map_err(|e| PackageError::write(directory, e))?;
        stage::sync_folder(directory).map_err(|e| PackageError::write(directory, e))?;
        stage::sync_folder(&crate::package_paths::parent_of(directory))
            .map_err(|e| PackageError::write(directory, e))?;
        self.check()?;
        self.check_files()?;
        ownership.ensure_writable(path, self.context.cancel_flag())?;
        self.context.writable(&self.path(wire::MANIFEST))
    }

    fn check_files(&self) -> PackageResult<()> {
        for (path, file) in &self.files {
            file.ensure_writable(path, self.context.cancel_flag())?;
        }
        Ok(())
    }

    pub(super) fn finish<T>(self, result: PackageResult<T>) -> PackageResult<T> {
        if self.published || result.is_ok() {
            return result;
        }
        error::finish(result, self.cleanup())
    }

    fn cleanup(&self) -> PackageResult<()> {
        let uncancelled = std::sync::atomic::AtomicBool::new(false);
        self.root
            .ensure_namespace_writable(self.root.path(), &uncancelled)?;
        for (path, ownership) in &self.files {
            ownership.remove(path, &uncancelled)?;
        }
        let remove_empty = |permit: &OwnedDirectoryPermit| -> PackageResult<()> {
            permit.ensure_namespace_writable(permit.path(), &uncancelled)?;
            fs::remove_dir(permit.path()).map_err(|e| PackageError::write(permit.path(), e))
        };
        if let Some(originals) = &self.originals {
            remove_empty(originals)?;
        }
        remove_empty(&self.root)
    }
}

fn write_manifest(path: &Path, file: File, manifest: &Manifest<'_>) -> PackageResult<()> {
    let mut output = BufWriter::new(file);
    serde_json::to_writer(&mut output, manifest)
        .map_err(|e| PackageError::write(path, e.into()))?;
    output
        .write_all(b"\n")
        .map_err(|e| PackageError::write(path, e))?;
    output
        .into_inner()
        .map_err(|e| PackageError::write(path, e.into_error()))?
        .sync_all()
        .map_err(|e| PackageError::write(path, e))
}

#[cfg(test)]
#[path = "player_bundle_output_tests.rs"]
mod tests;
