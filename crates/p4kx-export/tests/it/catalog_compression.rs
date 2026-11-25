// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The P4KX catalog written as plain JSON or as a zstd frame, through the public API.

use std::error::Error;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, UNIX_EPOCH};

use p4kx_export::{
    CancelToken, CatalogError, CatalogOptions, Compression, ZSTD_LEVEL, ZSTD_WINDOW_LOG,
    build_catalog, write_catalog_from,
};
use p4kx_extraction::{Archive, DataForgeEntry, ProvenanceMode};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestInstance, TestRecord, TestStruct, TestType,
    TestValue,
};

mod size_warning;

type TestResult = Result<(), Box<dyn Error>>;

const EXPORTED_AT: Duration = Duration::from_secs(1_792_000_000);
const MANUFACTURERS: u128 = 400;

fn database() -> Vec<u8> {
    let mut builder = DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new("SCItemLocalization")
                .field("Name", TestType::Locale)
                .field("Description", TestType::Locale),
        )
        .structure(
            TestStruct::new("SCItemManufacturer")
                .field("Code", TestType::String)
                .field(
                    "Localization",
                    TestType::Class("SCItemLocalization".to_owned()),
                ),
        );
    for number in 1..=MANUFACTURERS {
        let class = format!("M{number:04}");
        builder = builder.record(
            TestRecord::new(
                (number << 96) | 0x4000_8000_0000_0000_0000_0001,
                "SCItemManufacturer",
                format!("SCItemManufacturer.{class}"),
                format!("libs/foundry/records/scitemmanufacturer/{class}.xml"),
            )
            .value("Code", TestValue::text(&class))
            .value(
                "Localization",
                TestValue::Struct(
                    TestInstance::new("SCItemLocalization")
                        .value("Name", TestValue::text("@loc_placeholder"))
                        .value("Description", TestValue::text("@loc_placeholder")),
                ),
            ),
        );
    }
    builder.build()
}

pub(crate) fn archive(dir: &Path) -> Result<Archive, Box<dyn Error>> {
    let path = dir.join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data\\Game2.dcb", database()))
        .write_to(&path)?;
    Ok(Archive::open_with_key(
        &path,
        Some(p4kx_testkit::test_key()),
    )?)
}

pub(crate) fn options() -> CatalogOptions<'static> {
    CatalogOptions::new(UNIX_EPOCH + EXPORTED_AT)
}

#[test]
fn a_path_ending_in_zst_selects_zstd_and_any_other_path_plain_json() {
    let zstd = Compression::for_path(Path::new("out/p4kx-catalog.json.zst"));
    let upper = Compression::for_path(Path::new("CATALOG.JSON.ZST"));
    assert_eq!(zstd, Compression::ZSTD, "json.zst");
    assert_eq!(upper, Compression::ZSTD, "case is ignored");
    for plain in ["catalog.json", "catalog", "catalog.json.gz", "zst"] {
        assert_eq!(
            Compression::for_path(Path::new(plain)),
            Compression::None,
            "{plain}"
        );
    }
    assert_eq!(Compression::default(), Compression::None, "the default");
}

#[test]
fn a_zstd_catalog_decodes_to_the_bytes_of_the_plain_catalog() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(dir.path())?;
    let entry = DataForgeEntry::read(&archive)?;
    let plain = build_catalog(&archive, options().with_full_provenance())?;

    let (summary, packed) = write_catalog_from(
        &archive,
        &entry,
        options().with_full_provenance(),
        Compression::ZSTD,
        || Ok(Vec::new()),
    )?;

    assert!(
        packed.len() < plain.json().len() / 2,
        "{} of {} bytes",
        packed.len(),
        plain.json().len()
    );
    assert_eq!(
        summary.compressed_size(),
        Some(u64::try_from(packed.len())?),
        "compressed size"
    );
    assert_eq!(
        summary.size(),
        u64::try_from(plain.json().len())?,
        "uncompressed size"
    );
    assert_eq!(
        summary.upload_size(),
        summary.compressed_size().unwrap_or(0)
    );
    let mut decoded = Vec::new();
    zstd::Decoder::new(packed.as_slice())?.read_to_end(&mut decoded)?;
    assert!(decoded == plain.json(), "byte-identical JSON");
    Ok(())
}

struct FrameHeader {
    checksum: bool,
    content_size: Option<u64>,
    window: u64,
}

fn header(frame: &[u8]) -> Result<FrameHeader, Box<dyn Error>> {
    assert_eq!(
        frame.get(..4),
        Some(&[0x28, 0xb5, 0x2f, 0xfd][..]),
        "zstd magic"
    );
    let descriptor = *frame.get(4).ok_or("no frame header")?;
    let size_flag = descriptor >> 6;
    let single = descriptor & 0b10_0000 != 0;
    let mut at = 5;
    let mut window = None;
    if !single {
        let byte = *frame.get(at).ok_or("no window descriptor")?;
        let base = 1_u64 << (10 + u32::from(byte >> 3));
        window = Some(base + base / 8 * u64::from(byte & 7));
        at += 1;
    }
    let width = match (size_flag, single) {
        (0, false) => 0,
        (0, true) => 1,
        (1, _) => 2,
        (2, _) => 4,
        _ => 8,
    };
    let mut content = 0_u64;
    for (shift, byte) in frame
        .get(at..at + width)
        .ok_or("no content size")?
        .iter()
        .enumerate()
    {
        content |= u64::from(*byte) << (8 * shift);
    }
    if width == 2 {
        content += 256;
    }
    let content_size = (width > 0).then_some(content);
    Ok(FrameHeader {
        checksum: descriptor & 0b100 != 0,
        content_size,
        window: window.or(content_size).ok_or("no window")?,
    })
}

#[test]
fn the_frame_declares_its_content_size_a_matching_window_and_a_checksum() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(dir.path())?;
    let entry = DataForgeEntry::read(&archive)?;

    let (summary, packed) = write_catalog_from(
        &archive,
        &entry,
        options().with_full_provenance(),
        Compression::ZSTD,
        || Ok(Vec::new()),
    )?;

    let header = header(&packed)?;
    assert!(header.checksum, "content checksum flag");
    assert_eq!(
        header.content_size,
        Some(summary.size()),
        "the frame declares the JSON length"
    );
    assert!(
        header.window <= summary.size().next_power_of_two(),
        "the window {} follows the content size {}, not the 2^{ZSTD_WINDOW_LOG} maximum",
        header.window,
        summary.size()
    );
    let mut reader = zstd::Decoder::new(packed.as_slice())?;
    reader.window_log_max(ZSTD_WINDOW_LOG)?;
    let mut decoded = Vec::new();
    reader.read_to_end(&mut decoded)?;
    assert_eq!(decoded.last(), Some(&b'\n'), "ends in a newline");
    assert_eq!(ZSTD_LEVEL, 19, "level");
    assert_eq!(ZSTD_WINDOW_LOG, 27, "window");
    Ok(())
}

#[test]
fn a_plain_write_equals_the_built_json_and_has_no_compressed_size() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(dir.path())?;
    let entry = DataForgeEntry::read(&archive)?;
    let built = build_catalog(&archive, options().with_full_provenance())?;

    let (summary, bytes) = write_catalog_from(
        &archive,
        &entry,
        options().with_full_provenance(),
        Compression::None,
        || Ok(Vec::new()),
    )?;

    assert!(bytes == built.json(), "identical bytes");
    assert_eq!(summary.compressed_size(), None, "not compressed");
    assert_eq!(summary.upload_size(), summary.size(), "upload size");
    assert_eq!(summary, *built.summary(), "identical summary");
    Ok(())
}

#[test]
fn counting_a_catalog_to_a_sink_matches_the_complete_file_frame() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(dir.path())?;
    let entry = DataForgeEntry::read(&archive)?;
    let options = options();
    let (counted, _) = write_catalog_from(
        &archive,
        &entry,
        options.with_full_provenance(),
        Compression::ZSTD,
        || Ok(io::sink()),
    )?;
    let (buffered, frame) = write_catalog_from(
        &archive,
        &entry,
        options.with_full_provenance(),
        Compression::ZSTD,
        || Ok(Vec::new()),
    )?;
    let decoded = io::copy(&mut zstd::Decoder::new(frame.as_slice())?, &mut io::sink())?;
    assert_eq!(
        counted, buffered,
        "discarding output leaves the writer summary unchanged"
    );
    assert_eq!(
        counted.compressed_size(),
        Some(u64::try_from(frame.len())?),
        "exact encoded bytes"
    );
    assert_eq!(counted.size(), decoded, "exact decoded bytes");
    Ok(())
}

#[test]
fn the_catalog_is_streamed_in_pieces_not_in_one_write() -> TestResult {
    struct Chunks(Vec<usize>);
    impl Write for Chunks {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.0.push(bytes.len());
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let dir = tempfile::tempdir()?;
    let archive = archive(dir.path())?;
    let entry = DataForgeEntry::read(&archive)?;

    let (summary, chunks) = write_catalog_from(
        &archive,
        &entry,
        options().with_full_provenance(),
        Compression::None,
        || Ok(Chunks(Vec::new())),
    )?;

    let largest = chunks.0.iter().copied().max().unwrap_or(0);
    assert!(chunks.0.len() > 1, "several writes");
    assert!(
        u64::try_from(largest)? < summary.size(),
        "no write holds the whole catalog"
    );
    Ok(())
}

#[test]
fn nothing_is_created_when_the_build_fails() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(dir.path())?;
    let entry = DataForgeEntry::read(&archive)?;
    let cancel = CancelToken::new();
    cancel.cancel();
    let cancelled = CatalogOptions {
        cancel: Some(&cancel),
        ..options()
    };

    let result = write_catalog_from(&archive, &entry, &cancelled, Compression::ZSTD, || {
        Err::<Vec<u8>, _>(io::Error::other("must not be called"))
    });

    assert!(matches!(result, Err(CatalogError::Cancelled)), "cancelled");
    Ok(())
}

#[test]
fn a_writer_that_fails_gives_a_write_error() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(dir.path())?;
    let entry = DataForgeEntry::read(&archive)?;

    let options = options();
    for mode in [ProvenanceMode::MarkersOnly, ProvenanceMode::Full] {
        let output = options.with_provenance_mode(mode);
        let created = write_catalog_from(&archive, &entry, output, Compression::ZSTD, || {
            Err::<Vec<u8>, _>(io::Error::other("no space"))
        });
        let broken = write_catalog_from(&archive, &entry, output, Compression::None, || Ok(Broken));

        assert!(
            matches!(created, Err(CatalogError::Write(_))),
            "{mode:?} create"
        );
        assert!(
            matches!(broken, Err(CatalogError::Write(_))),
            "{mode:?} write"
        );
    }
    Ok(())
}

struct Broken;

impl Write for Broken {
    fn write(&mut self, _bytes: &[u8]) -> io::Result<usize> {
        Err(io::Error::other("disk full"))
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn a_file_is_written_to_a_path() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(dir.path())?;
    let entry = DataForgeEntry::read(&archive)?;
    let out: PathBuf = dir.path().join("catalog.json.zst");

    let (_, file) = write_catalog_from(
        &archive,
        &entry,
        options().with_full_provenance(),
        Compression::ZSTD,
        || fs::File::create(&out),
    )?;
    file.sync_all()?;

    let mut decoded = Vec::new();
    zstd::Decoder::new(fs::File::open(&out)?)?.read_to_end(&mut decoded)?;
    assert!(decoded.starts_with(b"{\"schemaVersion\":3"), "JSON");
    Ok(())
}
