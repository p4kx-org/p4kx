// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::Write;

use anyhow::Result;
use p4kx_blender::PlayerSource;
use p4kx_extraction::{ArchiveWriteProtection, is_same_file, write_replacing_io_protected};

use crate::cli_character::CharacterSourceArgs;
use crate::entity_glb::GlbOutputIsArchive;
use crate::extract::cancel_on_interrupt;

pub(super) fn run(args: &CharacterSourceArgs) -> Result<()> {
    if is_same_file(&args.out, &args.archive) {
        return Err(GlbOutputIsArchive(args.out.clone()).into());
    }
    let archive = crate::key::open(&args.archive)?;
    let cancel = cancel_on_interrupt()?;
    let protection = ArchiveWriteProtection::new(archive.path(), cancel.flag())?;
    let data = PlayerSource::read(&archive, args.body.into(), &cancel)?;
    write_replacing_io_protected(&args.out, Some(&protection), Some(&cancel), |writer| {
        serde_json::to_writer(&mut *writer, &data)?;
        writeln!(writer)
    })?;
    Ok(())
}
