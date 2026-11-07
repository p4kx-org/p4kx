// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Directory timestamp retention through independent ZIP, v1, v2 and recovery bytes.

mod fixed_header_fixture;
mod fixed_headers;
mod fixture;
mod v2;
mod zip;

use std::error::Error;
use std::io::Read as _;

use p4kx_archive::{Archive, Checksum, DosCalendar, EntryIssue, InstallationStatus, OpenOptions};
use p4kx_testkit::{ArchiveBuilder, TestEntry};

use fixture::{FixtureResult, Words};

type TestResult = Result<(), Box<dyn Error>>;

#[derive(Debug)]
struct Case {
    words: Words,
    calendar: Option<[u16; 6]>,
}

const CASES: &[Case] = &[
    Case {
        words: Words { date: 0, time: 0 },
        calendar: None,
    },
    Case {
        words: Words {
            date: 0x0021,
            time: 0,
        },
        calendar: Some([1980, 1, 1, 0, 0, 0]),
    },
    Case {
        words: Words {
            date: 0x285d,
            time: 0x5021,
        },
        calendar: Some([2000, 2, 29, 10, 1, 2]),
    },
    Case {
        words: Words {
            date: 0x585d,
            time: 0xbf7d,
        },
        calendar: Some([2024, 2, 29, 23, 59, 58]),
    },
    Case {
        words: Words {
            date: 0xf05d,
            time: 0x5021,
        },
        calendar: None,
    },
    Case {
        words: Words {
            date: 0xf05c,
            time: 0xbf7d,
        },
        calendar: Some([2100, 2, 28, 23, 59, 58]),
    },
    Case {
        words: Words {
            date: 0xff9f,
            time: 0xbf7d,
        },
        calendar: Some([2107, 12, 31, 23, 59, 58]),
    },
    Case {
        words: Words {
            date: 0x2a5d,
            time: 0x5021,
        },
        calendar: None,
    },
    Case {
        words: Words {
            date: 0x5801,
            time: 0,
        },
        calendar: None,
    },
    Case {
        words: Words {
            date: 0x59a1,
            time: 0,
        },
        calendar: None,
    },
    Case {
        words: Words {
            date: 0x5820,
            time: 0,
        },
        calendar: None,
    },
    Case {
        words: Words {
            date: 0x589f,
            time: 0,
        },
        calendar: None,
    },
    Case {
        words: Words {
            date: 0x585e,
            time: 0,
        },
        calendar: None,
    },
    Case {
        words: Words {
            date: 0x585d,
            time: 0xc000,
        },
        calendar: None,
    },
    Case {
        words: Words {
            date: 0x585d,
            time: 0x0780,
        },
        calendar: None,
    },
    Case {
        words: Words {
            date: 0x585d,
            time: 0x001e,
        },
        calendar: None,
    },
    Case {
        words: Words {
            date: 0x585d,
            time: 0x001f,
        },
        calendar: None,
    },
    Case {
        words: Words {
            date: 0xffff,
            time: 0xffff,
        },
        calendar: None,
    },
    Case {
        words: Words {
            date: 0x585d,
            time: 0,
        },
        calendar: Some([2024, 2, 29, 0, 0, 0]),
    },
    Case {
        words: Words {
            date: 0,
            time: 0xbf7d,
        },
        calendar: None,
    },
];

fn words() -> Vec<Words> {
    CASES.iter().map(|case| case.words).collect()
}

fn fields(calendar: DosCalendar) -> [u16; 6] {
    [
        calendar.year(),
        u16::from(calendar.month()),
        u16::from(calendar.day()),
        u16::from(calendar.hour()),
        u16::from(calendar.minute()),
        u16::from(calendar.second()),
    ]
}

fn open(bytes: &[u8], options: OpenOptions) -> FixtureResult<(tempfile::TempDir, Archive)> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("fixture.p4k");
    std::fs::write(&path, bytes)?;
    let archive = Archive::open_with(&path, options)?;
    Ok((directory, archive))
}

fn check_records(archive: &Archive, first_index: usize) -> TestResult {
    let entries = archive
        .entries()
        .get(first_index..)
        .ok_or("timestamp Entries are missing")?;
    assert_eq!(
        entries.len(),
        CASES.len(),
        "every physical timestamp record remains present"
    );
    for (index, (entry, case)) in entries.iter().zip(CASES).enumerate() {
        let timestamp = entry.metadata().dos_timestamp();
        assert_eq!(
            timestamp.raw_date(),
            case.words.date,
            "source date bits for {case:?}"
        );
        assert_eq!(
            timestamp.raw_time(),
            case.words.time,
            "source time bits for {case:?}"
        );
        assert_eq!(
            timestamp.calendar().map(fields),
            case.calendar,
            "independently authored calendar for {case:?}"
        );
        assert_eq!(
            entry.index(),
            index + first_index,
            "metadata does not change admitted Entry indices"
        );
        let mut payload = Vec::new();
        archive
            .open_entry_verified(entry)?
            .read_to_end(&mut payload)?;
        assert_eq!(
            payload, b"abc",
            "invalid calendar values do not alter payload verification"
        );
    }
    Ok(())
}

#[test]
fn v1_and_plain_zip_retain_cdr_words_instead_of_local_header_words() -> TestResult {
    for layout in [zip::Layout::P4k, zip::Layout::Zip] {
        let fixture = zip::fixture(&words(), layout)?;
        let (_directory, archive) = open(&fixture.bytes, OpenOptions::new())?;
        check_records(&archive, 1)?;
        assert!(
            archive.issues().is_empty(),
            "calendar decoding is not an Archive-integrity diagnostic"
        );
        assert!(
            matches!(
                archive.installation_status(),
                InstallationStatus::Unavailable
            ),
            "timestamp retention does not invent an installation declaration"
        );
        let entry = archive.entries().last().ok_or("last timestamp Entry")?;
        let expected = match layout {
            zip::Layout::P4k => Checksum::Crc32c(crc32c::crc32c(b"abc")),
            zip::Layout::Zip => Checksum::Crc32(crc32fast::hash(b"abc")),
        };
        assert_eq!(
            entry.checksum(),
            expected,
            "the layout's existing CRC interpretation is unchanged"
        );
    }
    Ok(())
}

#[test]
fn v2_base_extension_and_extension_only_records_retain_all_timestamp_words() -> TestResult {
    let stamps = words();
    for base in [stamps.len(), 1, 0] {
        let (_directory, archive) = open(&v2::fixture(&stamps, base)?, OpenOptions::new())?;
        check_records(&archive, 0)?;
        assert!(
            archive.issues().is_empty(),
            "invalid stored calendars do not change source completeness"
        );
        assert!(
            matches!(
                archive.installation_status(),
                InstallationStatus::DeclaredComplete { flags: 0 }
            ),
            "the declared installation status is preserved"
        );
        assert!(
            archive
                .entries()
                .iter()
                .all(|entry| entry.installed_bytes() == Some(3)),
            "timestamp metadata does not replace install progress"
        );
    }
    Ok(())
}

#[test]
fn recovery_retains_timestamp_bits_without_hiding_the_recovered_inventory_marker() -> TestResult {
    for layout in [zip::Layout::P4k, zip::Layout::Zip] {
        let mut fixture = zip::fixture(&words(), layout)?;
        fixture.bytes.truncate(fixture.directory_end);
        let (_directory, archive) = open(&fixture.bytes, OpenOptions::new().recover(true))?;
        check_records(&archive, 1)?;
        assert_eq!(
            archive.issues(),
            [(0, EntryIssue::Recovered)],
            "valid and invalid calendar views retain the authoritative recovery diagnostic"
        );
    }
    Ok(())
}

#[test]
fn nested_zip_metadata_survives_folder_removal_and_file_reindexing() -> TestResult {
    let bytes = zip::fixture(&words(), zip::Layout::Zip)?.bytes;
    for entry in [
        TestEntry::stored("Data\\test.socpak", bytes.clone()),
        TestEntry::deflated("Data\\test.socpak", bytes),
    ] {
        let parent_bytes = ArchiveBuilder::new().entry(entry).build();
        let (_directory, parent) = open(&parent_bytes, OpenOptions::new())?;
        let outer = parent.entries().first().ok_or("outer container")?;
        let archive = parent.open_container(outer)?;
        check_records(&archive, 0)?;
        assert!(
            archive.issues().is_empty(),
            "admission retains timestamp metadata without adding diagnostics"
        );
        assert_eq!(
            archive
                .entries()
                .first()
                .ok_or("first admitted Entry")?
                .path(),
            "Data\\item-0.bin",
            "the dropped folder does not shift a file's metadata ownership"
        );
    }
    Ok(())
}
