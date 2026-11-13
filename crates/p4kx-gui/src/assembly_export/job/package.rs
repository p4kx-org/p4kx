// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{ArchiveModels, Assembly, Interior, LoadoutChanges};
use p4kx_blender::{
    BuildInfo, InteriorScene, PackageError, PackageInfo, PackageSummary, PackageWriteContext,
    Replacement, write_interior_package_with_context, write_package_with_context,
};
use p4kx_channels::channel_name;
use p4kx_extraction::{ArchiveWriteProtection, CancelToken, WriteProtectionError};

use super::failure_of;
use super::stop::{Stoppable, StoppableTextures};
use super::{Ended, Request};
use crate::assembly_export::AssemblyExportSummary;
use crate::{EntryIndex, ModelExportFailure};

/// Writes `assembly` as the Blender package of `request`, replacing an earlier package of the
/// same folder only once the new one is complete.
pub(super) fn write(
    index: &EntryIndex,
    request: &Request,
    (assembly, models): (&Assembly, &ArchiveModels<'_>),
    (changes, cancel): (&LoadoutChanges, &CancelToken),
) -> Ended {
    let provider = Stoppable { models, cancel };
    let loader = models.textures();
    let textures = StoppableTextures {
        textures: &loader,
        cancel,
    };
    let info = info_of(index, changes);
    let protection = match ArchiveWriteProtection::new(index.path(), cancel.flag()) {
        Ok(protection) => protection,
        Err(error) if error.is_cancelled() => return Ended::Cancelled,
        Err(error) => return Ended::Failed(ModelExportFailure::Refused(error)),
    };
    let context =
        PackageWriteContext::new(&protection, cancel.flag()).with_archive_texture_source(&loader);
    let written = write_package_with_context(
        assembly,
        &provider,
        &textures,
        &request.options(),
        &info,
        &request.file,
        &context,
    );
    ended(index, cancel, written, |summary| {
        AssemblyExportSummary::of_package(
            (request.file.clone(), request.name.clone()),
            assembly,
            (summary, changes),
        )
    })
}

/// Writes `interior` as the Blender package of `request`, replacing an earlier package of the
/// same folder only once the new one is complete.
pub(super) fn write_interior(
    index: &EntryIndex,
    request: &Request,
    interior: &Interior,
    (models, cancel): (&ArchiveModels<'_>, &CancelToken),
) -> Ended {
    let provider = Stoppable { models, cancel };
    let loader = models.textures();
    let textures = StoppableTextures {
        textures: &loader,
        cancel,
    };
    let info = info_of(index, &LoadoutChanges::default());
    let protection = match ArchiveWriteProtection::new(index.path(), cancel.flag()) {
        Ok(protection) => protection,
        Err(error) if error.is_cancelled() => return Ended::Cancelled,
        Err(error) => return Ended::Failed(ModelExportFailure::Refused(error)),
    };
    let context =
        PackageWriteContext::new(&protection, cancel.flag()).with_archive_texture_source(&loader);
    let written = write_interior_package_with_context(
        &InteriorScene::new(&request.name, interior),
        &provider,
        &textures,
        &request.options(),
        &info,
        &request.file,
        &context,
    );
    ended(index, cancel, written, |summary| {
        AssemblyExportSummary::of_interior_package(
            (request.file.clone(), request.name.clone()),
            interior,
            summary,
        )
    })
}

fn ended(
    index: &EntryIndex,
    cancel: &CancelToken,
    written: Result<PackageSummary, PackageError>,
    summary: impl FnOnce(PackageSummary) -> AssemblyExportSummary,
) -> Ended {
    match written {
        Ok(written) => Ended::Written(Box::new(summary(written))),
        Err(error) => package_failure(index, error, cancel.is_cancelled())
            .map_or(Ended::Cancelled, Ended::Failed),
    }
}

/// Returns the package failure, or `None` when the operation was cancelled.
pub(crate) fn package_failure(
    index: &EntryIndex,
    error: PackageError,
    cancelled: bool,
) -> Option<ModelExportFailure> {
    match verdict(error, cancelled) {
        Verdict::Cancelled => None,
        Verdict::Refused(refused) => Some(ModelExportFailure::Refused(refused)),
        Verdict::Failed(error) => Some(failure_of(index, &error)),
    }
}

#[derive(Debug)]
enum Verdict {
    Cancelled,
    Refused(WriteProtectionError),
    Failed(PackageError),
}

/// Preserves typed failures; ordinary writes after cancellation end as cancellation.
fn verdict(error: PackageError, cancelled: bool) -> Verdict {
    match error {
        PackageError::Cancelled => Verdict::Cancelled,
        PackageError::Refused(refused) => Verdict::Refused(refused),
        PackageError::Write { .. } if cancelled => Verdict::Cancelled,
        error => Verdict::Failed(error),
    }
}

pub(crate) fn info_of(index: &EntryIndex, changes: &LoadoutChanges) -> PackageInfo {
    let patches = crate::public_patches::current();
    let build = index.build_of_archive().map(|build| BuildInfo {
        branch: build.branch().to_owned(),
        version: build.version().to_owned(),
        public_patch: patches.of(build).map(str::to_owned),
        change_number: build.change_number().to_owned(),
        build_id: build.build_id().to_owned(),
    });
    let replace = changes
        .replacements()
        .iter()
        .map(|(port, entity)| Replacement {
            port: port.clone(),
            entity: entity.clone(),
        })
        .collect();
    PackageInfo::new(channel_name(index.path()), build)
        .with_changes(changes.exclusions().to_vec(), replace)
}

#[cfg(test)]
mod tests {
    use std::io;
    use std::path::PathBuf;

    use super::*;

    fn not_restored() -> PackageError {
        PackageError::NotRestored {
            path: PathBuf::from("out"),
            old: PathBuf::from("out.old"),
            source: io::Error::other("locked"),
        }
    }

    #[test]
    fn a_package_that_could_not_be_restored_is_a_failure_even_after_a_cancel() {
        assert!(matches!(
            verdict(not_restored(), true),
            Verdict::Failed(PackageError::NotRestored { .. })
        ));
    }

    #[test]
    fn cleanup_failure_remains_visible_after_a_cancel() {
        let error = PackageError::Cleanup(p4kx_extraction::OwnedOutputCleanupError::Io {
            path: PathBuf::from("out.stage"),
            source: io::Error::other("cleanup failed"),
        });
        assert!(
            matches!(
                verdict(error, true),
                Verdict::Failed(PackageError::Cleanup(_))
            ),
            "a retained temporary output is an operation failure"
        );
    }

    #[test]
    fn refused_restoration_remains_visible_after_a_cancel() {
        let error = PackageError::RestoreRefused {
            path: PathBuf::from("out"),
            old: PathBuf::from("out.old"),
            cause: Box::new(PackageError::Refused(
                WriteProtectionError::SourceOverwrite {
                    archive: PathBuf::from("source.p4k"),
                    target: PathBuf::from("out.old"),
                },
            )),
            replacement: Box::new(PackageError::Cancelled),
        };
        assert!(
            matches!(
                verdict(error, true),
                Verdict::Failed(PackageError::RestoreRefused { .. })
            ),
            "an unrestored earlier package is an operation failure"
        );
    }

    #[test]
    fn a_write_error_after_a_cancel_is_the_cancel() {
        let error = PackageError::Write {
            path: PathBuf::from("out"),
            source: io::Error::other("stopped"),
        };

        assert!(matches!(verdict(error, true), Verdict::Cancelled));
    }

    #[test]
    fn a_write_error_without_a_cancel_is_a_failure() {
        let error = PackageError::Write {
            path: PathBuf::from("out"),
            source: io::Error::other("full"),
        };

        assert!(matches!(verdict(error, false), Verdict::Failed(_)));
    }

    #[test]
    fn the_writers_own_cancel_is_a_cancel() {
        assert!(matches!(
            verdict(PackageError::Cancelled, false),
            Verdict::Cancelled
        ));
    }
}
