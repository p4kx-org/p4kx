// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{ArchiveFiles, LoadoutFiles};
use p4kx_extraction::{Archive, ArchiveWatch, DataForge, DataForgeEntry};

/// Holds one unchanged Archive and DataForge open throughout a verification callback.
pub(crate) fn with_source(
    verify: impl for<'f> FnOnce(&Archive, &'f DataForge<'f>, &'f dyn LoadoutFiles) -> super::Result<()>,
) -> super::Result<()> {
    let archive = Archive::open_with_key(
        p4kx_testkit::real_archive_path()?,
        Some(p4kx_testkit::real_archive_key()?),
    )?;
    let watch = ArchiveWatch::new(&archive);
    watch.check()?;
    let files = ArchiveFiles::new(&archive);
    let loaded = DataForgeEntry::read(&archive)?;
    watch.check()?;
    let forge = loaded.open()?;
    let result = verify(&archive, &forge, &files);
    watch.check()?;
    result
}
