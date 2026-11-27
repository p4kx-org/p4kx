// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The P4KX catalog of a real Archive written as zstd at levels 15, 17 and 19: sizes, ratios
//! and times, reference size thresholds, and the decoded bytes against the plain catalog; runs
//! with `--ignored` when `P4KX_DATA_P4K` names an Archive. The level 19 frame is left in the
//! temporary folder as `p4kx-catalog-<channel>.json.zst`, so that `zstd -d --long=27` can
//! check it.

use std::error::Error;
use std::fs;
use std::io::Read;
use std::time::{Duration, Instant, SystemTime};

use p4kx_export::{
    CatalogOptions, Compression, ZSTD_LEVEL, ZSTD_WINDOW_LOG, build_catalog_from,
    write_catalog_from,
};
use p4kx_extraction::{Archive, DataForgeEntry};
use p4kx_testkit::real_archive_path;

type TestResult = Result<(), Box<dyn Error>>;

const LEVELS: [i32; 3] = [15, 17, ZSTD_LEVEL];

#[expect(
    clippy::print_stdout,
    reason = "the sizes and times are the measurement the level is chosen from"
)]
fn print_line(line: &str) {
    println!("{line}");
}

#[expect(
    clippy::cast_precision_loss,
    reason = "a ratio of sizes far below 2^52"
)]
fn ratio(size: u64, compressed: u64) -> f64 {
    size as f64 / compressed.max(1) as f64
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn the_catalog_of_a_real_archive_round_trips_at_every_compression_level() -> TestResult {
    let path = real_archive_path()?;
    let archive = Archive::open_with_key(&path, Some(p4kx_testkit::real_archive_key()?))?;
    let entry = DataForgeEntry::read(&archive)?;
    let options = CatalogOptions::new(SystemTime::now());
    let started = Instant::now();
    let plain = build_catalog_from(&archive, &entry, options.with_full_provenance())?;
    let plain_took = started.elapsed();
    let channel = p4kx_channels::channel_name(&path).unwrap_or_default();
    print_line(&format!(
        "{channel}: {} bytes uncompressed, window log {ZSTD_WINDOW_LOG}",
        plain.json().len()
    ));
    crate::real_archive::check_transport(plain.summary(), u64::try_from(plain.json().len())?, None);

    for level in LEVELS {
        let started = Instant::now();
        let (summary, frame) = write_catalog_from(
            &archive,
            &entry,
            options.with_full_provenance(),
            Compression::Zstd { level },
            || Ok(Vec::new()),
        )?;
        let took = started.elapsed();
        let compressed = u64::try_from(frame.len())?;
        report(
            &channel,
            level,
            (plain.summary().size(), compressed),
            (plain_took, took),
        );

        let mut reader = zstd::Decoder::new(frame.as_slice())?;
        reader.window_log_max(ZSTD_WINDOW_LOG)?;
        let mut json = Vec::new();
        reader.read_to_end(&mut json)?;
        crate::real_archive::check_transport(
            &summary,
            u64::try_from(json.len())?,
            Some(compressed),
        );
        assert_eq!(json.len(), plain.json().len(), "decoded length");
        assert!(json == plain.json(), "byte-identical JSON at level {level}");
        if level == ZSTD_LEVEL {
            let file = std::env::temp_dir().join(format!("p4kx-catalog-{channel}.json.zst"));
            fs::write(&file, &frame)?;
            print_line(&format!("level {level} frame: {}", file.display()));
        }
    }
    Ok(())
}

fn report(
    channel: &str,
    level: i32,
    (size, compressed): (u64, u64),
    (plain_took, took): (Duration, Duration),
) {
    print_line(&format!(
        "{channel} level {level}: {size} to {compressed} bytes, ratio {:.1}, {:.2} s to build and compress, {:.2} s to build plain, about {:.2} s to compress",
        ratio(size, compressed),
        took.as_secs_f64(),
        plain_took.as_secs_f64(),
        took.saturating_sub(plain_took).as_secs_f64()
    ));
}
