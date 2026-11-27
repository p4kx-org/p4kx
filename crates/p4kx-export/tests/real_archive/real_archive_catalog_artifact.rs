// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::{self, File};
use std::io::{self, BufReader, Read};
use std::path::{Path, PathBuf};

use p4kx_export::ZSTD_WINDOW_LOG;
use serde::Deserialize;

use super::{sections, support};
use crate::real_archive_catalog_pipe::CountingReader;

const FRAME: &str = "p4kx-catalog.json.zst";
const RAW: &str = "p4kx-catalog.json";

#[derive(Deserialize)]
struct Measurement {
    status: String,
    mode: String,
    sizes: Sizes,
}

#[derive(Deserialize)]
struct Sizes {
    #[serde(rename = "encodedBytes")]
    encoded: u64,
    #[serde(rename = "decodedBytes")]
    decoded: u64,
    #[serde(rename = "fileBytes")]
    file: u64,
}

#[test]
#[ignore = "completed private catalog measurement: set P4KX_CATALOG_MEASUREMENT_DIR"]
fn reconcile_completed_catalog_artifact() -> support::TestResult {
    let directory = std::env::var_os("P4KX_CATALOG_MEASUREMENT_DIR")
        .map(PathBuf::from)
        .ok_or("P4KX_CATALOG_MEASUREMENT_DIR is required")?;
    let measurement = Measurement::load(&directory)?;
    let census = reconcile(&directory, &measurement.sizes)?;
    support::print_line(&format!(
        "catalog artifact {}",
        serde_json::to_string(&census)?
    ));
    Ok(())
}

impl Measurement {
    fn load(directory: &Path) -> support::TestResult<Self> {
        let receipt = File::open(directory.join("receipt.private.json"))?;
        let measurement: Self = serde_json::from_reader(BufReader::new(receipt))?;
        if measurement.status != "complete" || measurement.mode != "normal" {
            return Err("the measurement receipt must describe a completed normal export".into());
        }
        Ok(measurement)
    }
}

fn open_frame(directory: &Path, sizes: &Sizes) -> support::TestResult<File> {
    let frame = File::open(directory.join(FRAME))?;
    if frame.metadata()?.len() != sizes.encoded || sizes.file != sizes.encoded {
        return Err("compressed file length differs from the producer receipt".into());
    }
    Ok(frame)
}

pub(crate) fn read_completed<T>(
    directory: &Path,
    consume: impl FnOnce(&mut dyn Read) -> support::TestResult<T>,
) -> support::TestResult<T> {
    let (bytes, decoder) = open_completed_input(directory)?;
    let mut input = BufReader::new(CountingReader::new(decoder));
    let projection = consume(&mut input)?;
    if input.read(&mut [0])? != 0 || input.into_inner().byte_count() != bytes {
        return Err("decoded catalog length differs from the completed measurement".into());
    }
    Ok(projection)
}

pub(crate) fn read_completed_prefix<T>(
    directory: &Path,
    consume: impl FnOnce(&mut dyn Read) -> support::TestResult<T>,
) -> support::TestResult<T> {
    let (_, decoder) = open_completed_input(directory)?;
    consume(&mut BufReader::new(decoder))
}

fn open_completed_input(directory: &Path) -> support::TestResult<(u64, impl Read)> {
    let measurement = Measurement::load(directory)?;
    let frame = open_frame(directory, &measurement.sizes)?;
    let mut decoder = zstd::stream::read::Decoder::new(frame)?;
    decoder.window_log_max(ZSTD_WINDOW_LOG)?;
    let bytes = measurement.sizes.decoded;
    let limit = bytes
        .checked_add(1)
        .ok_or("decoded measurement length overflow")?;
    Ok((bytes, decoder.take(limit)))
}

fn reconcile(directory: &Path, sizes: &Sizes) -> support::TestResult<sections::Census> {
    let frame = open_frame(directory, sizes)?;
    let raw = directory.join(RAW);
    match fs::symlink_metadata(&raw) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
        Ok(_) => return Err("the raw catalog already exists".into()),
    }
    let mut staged = tempfile::NamedTempFile::new_in(directory)?;
    let census = sections::decode_to(frame, staged.as_file_mut())?;
    census.reconcile_sizes(sizes.encoded, sizes.decoded)?;
    if staged.as_file().metadata()?.len() != sizes.decoded {
        return Err("raw file length differs from the reconciled decoded size".into());
    }
    staged.persist_noclobber(raw).map_err(|error| error.error)?;
    Ok(census)
}

#[path = "real_archive_catalog_artifact_tests.rs"]
mod tests;
