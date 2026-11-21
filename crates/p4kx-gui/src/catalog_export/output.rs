// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogError, CatalogFile};

/// Removes an unpublished catalog while preserving its original failure and cleanup failure.
pub(super) fn discard(file: CatalogFile<'_>, primary: CatalogError) -> CatalogError {
    match file.discard() {
        Ok(()) => primary,
        Err(cleanup) => CatalogError::CleanupFailed {
            primary: Box::new(primary),
            cleanup: Box::new(cleanup),
        },
    }
}

#[cfg(test)]
mod tests {
    use std::io;

    use p4kx_export::{CancelToken, CatalogError, CatalogFile};
    use p4kx_extraction::ArchiveWriteProtection;

    #[test]
    fn cancelled_catalog_discards_the_unpublished_file_and_preserves_the_target() -> io::Result<()>
    {
        use std::io::Write as _;

        let folder = tempfile::tempdir()?;
        let source = folder.path().join("source.p4k");
        let target = folder.path().join("catalog.json");
        std::fs::write(&source, b"synthetic-source")?;
        std::fs::write(&target, b"earlier catalog")?;
        let cancel = CancelToken::new();
        let protection =
            ArchiveWriteProtection::new(&source, cancel.flag()).map_err(io::Error::other)?;
        let mut file = CatalogFile::create_protected(&target, &protection, &cancel)?;
        file.write_all(b"unpublished catalog")?;
        cancel.cancel();
        assert!(matches!(
            super::discard(file, CatalogError::Cancelled),
            CatalogError::Cancelled
        ));
        assert_eq!(std::fs::read(&target)?, b"earlier catalog");
        assert_eq!(std::fs::read(&source)?, b"synthetic-source");
        assert_eq!(
            std::fs::read_dir(folder.path())?.count(),
            2,
            "no temporary file remains"
        );
        Ok(())
    }

    #[test]
    fn refused_cleanup_preserves_both_catalog_failure_and_foreign_temporary_name() -> io::Result<()>
    {
        use std::io::Write as _;

        let folder = tempfile::tempdir()?;
        let source = folder.path().join("source.p4k");
        let target = folder.path().join("catalog.json");
        std::fs::write(&source, b"synthetic-source")?;
        std::fs::write(&target, b"earlier catalog")?;
        let cancel = CancelToken::new();
        let protection =
            ArchiveWriteProtection::new(&source, cancel.flag()).map_err(io::Error::other)?;
        let mut file = CatalogFile::create_protected(&target, &protection, &cancel)?;
        file.write_all(b"unpublished catalog")?;
        file.flush()?;
        let temporary = std::fs::read_dir(folder.path())?
            .map(|entry| entry.map(|entry| entry.path()))
            .find(|entry| {
                entry
                    .as_ref()
                    .map_or(true, |path| path != &source && path != &target)
            })
            .transpose()?
            .ok_or_else(|| io::Error::other("catalog factory created no temporary file"))?;
        let retained = folder.path().join("retained-owned-file");
        std::fs::rename(&temporary, &retained)?;
        std::fs::write(&temporary, b"foreign file")?;
        cancel.cancel();
        let error = super::discard(file, CatalogError::Cancelled);
        assert!(
            matches!(error, CatalogError::CleanupFailed { primary, .. } if matches!(*primary, CatalogError::Cancelled)),
            "failed cleanup must retain its original cancellation"
        );
        assert_eq!(std::fs::read(&temporary)?, b"foreign file");
        assert_eq!(std::fs::read(&retained)?, b"unpublished catalog");
        assert_eq!(std::fs::read(&target)?, b"earlier catalog");
        assert_eq!(std::fs::read(&source)?, b"synthetic-source");
        Ok(())
    }
}
