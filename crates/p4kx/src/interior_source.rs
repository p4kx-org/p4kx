// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::Write;

use anyhow::{Result, anyhow};
use p4kx_assembly::SocContainerCapture;
use p4kx_blender::SocScene;
use p4kx_extraction::{ArchiveWriteProtection, is_same_file, write_replacing_io_protected};

use crate::cli_interior::InteriorSourceArgs;
use crate::interior_glb::{InteriorOutputIsArchive, open};

/// Writes the versioned, authored SOC scene without building render geometry.
pub(crate) fn run(args: &InteriorSourceArgs) -> Result<()> {
    if is_same_file(&args.out, &args.archive) {
        return Err(InteriorOutputIsArchive(args.out.clone()).into());
    }
    let (archives, _) = open(&args.archive, &args.containers)?;
    let outer = archives
        .first()
        .ok_or_else(|| anyhow!("the Archive was not opened"))?;
    let cancel = crate::extract::cancel_on_interrupt()?;
    let protection = ArchiveWriteProtection::new(outer.path(), cancel.flag())?;
    let layers: Vec<_> = archives.iter().collect();
    let capture = SocContainerCapture::read(&layers, &cancel)?;
    let scene = SocScene::from_archive_capture(&capture, outer, &cancel)?;
    write_replacing_io_protected(&args.out, Some(&protection), Some(&cancel), |writer| {
        serde_json::to_writer(&mut *writer, &scene)?;
        writeln!(writer)
    })?;
    Ok(())
}
