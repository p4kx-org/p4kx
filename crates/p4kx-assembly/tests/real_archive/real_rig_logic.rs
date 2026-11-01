// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The `RigLogic` chunks of the head skins the game ships: the stored hash of every control is the
//! CRC-32 of its name.

use std::error::Error;
use std::io::Read;

use p4kx_character::RigLogic;
use p4kx_chunks::{ChunkFile, ivo};
use p4kx_extraction::Archive;
use p4kx_testkit::{real_archive_key, real_archive_path};

use crate::real_paths::unified;

type TestResult = Result<(), Box<dyn Error>>;

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
#[expect(clippy::print_stdout, reason = "the count is the measurement")]
fn the_stored_control_hashes_of_every_rig_logic_chunk_are_the_crcs_of_the_names() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, real_archive_key().ok())?;
    let (mut checked, mut unparsed) = (0_usize, 0_usize);
    for entry in archive.entries() {
        let path = unified(entry.path());
        let is_skin = std::path::Path::new(&path)
            .extension()
            .is_some_and(|extension| extension == "skin");
        if !is_skin {
            continue;
        }
        let mut bytes = Vec::new();
        archive.open_entry(entry)?.read_to_end(&mut bytes)?;
        let Ok(chunks) = ChunkFile::parse(bytes.as_slice()) else {
            unparsed += 1;
            continue;
        };
        let Some((index, _)) = chunks.chunks_of(ivo::RIG_LOGIC).next() else {
            continue;
        };
        let data = chunks
            .chunk_data(index)
            .ok_or("the chunk lies in the file")?;
        let logic = RigLogic::read(data).map_err(|error| format!("{path}: {error}"))?;
        assert_ne!(logic.controls(), 0, "{path}");
        checked += 1;
    }
    println!("{checked} skins with a RigLogic chunk, {unparsed} skins that do not parse");
    assert_ne!(checked, 0);
    Ok(())
}
