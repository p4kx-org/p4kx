// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Conversion of a folder of extracted files: the same files `extract` with Conversion writes.

use std::collections::BTreeMap;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use p4kx_extraction::{
    Archive, ConflictPolicy, EntryFailure, Exclusion, ExtractOptions, ExtractionReport, Filter,
    FolderError, FolderFileError, FolderSource, convert_folder, extract,
};
use p4kx_testkit::{
    ArchiveBuilder, CryXmlByteOrder, CryXmlSignature, DataForgeBuilder, SplitDds, TestCodec,
    TestElement, TestEntry, TestRecord, TestStruct, TestTextureFormat, TestType, TestValue,
    TestWem, test_signal,
};

#[path = "folder_dds_origins.rs"]
mod dds_origins;

type TestResult = Result<(), Box<dyn Error>>;

const TEXTURE: &str = "Data\\Textures\\rock_diff.dds";

fn ship() -> Vec<u8> {
    TestElement::new("Ship")
        .attribute("name", "Gladius")
        .to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little)
}

fn database() -> Vec<u8> {
    DataForgeBuilder::new()
        .structure(TestStruct::new("Item").field("count", TestType::Int32))
        .record(
            TestRecord::new(1, "Item", "Item.One", "libs/items/one.xml")
                .value("count", TestValue::Int32(7)),
        )
        .build()
}

fn sound() -> Vec<u8> {
    TestWem::pcm(TestCodec::WwisePcm, 1, 48_000, &test_signal(480, 1)).bytes()
}

fn texture() -> SplitDds {
    SplitDds::new(TestTextureFormat::LegacyDxt5, 32, 16).split(3)
}

fn entries() -> Vec<TestEntry> {
    let mut entries = texture().entries(TEXTURE);
    entries.extend([
        TestEntry::stored("Data\\Ships\\gladius.xml", ship()),
        TestEntry::stored("Data\\Ships\\text.xml", b"<a b='1'/>".to_vec()),
        TestEntry::stored("Data\\Sounds\\Media\\5.wem", sound()),
        TestEntry::stored("Data\\Game2.dcb", database()),
        TestEntry::stored("Data\\readme.bin", b"plain".to_vec()),
    ]);
    entries
}

struct Fixture {
    dir: tempfile::TempDir,
    archive: Archive,
}

impl Fixture {
    fn new() -> Result<Self, Box<dyn Error>> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("Data.p4k");
        entries()
            .into_iter()
            .fold(ArchiveBuilder::new(), ArchiveBuilder::entry)
            .write_to(&path)?;
        Ok(Self {
            archive: Archive::open_with_key(&path, Some(p4kx_testkit::test_key()))?,
            dir,
        })
    }

    fn folder(&self, name: &str) -> PathBuf {
        self.dir.path().join(name)
    }

    fn extract(&self, name: &str, convert: bool) -> Result<PathBuf, Box<dyn Error>> {
        let target = self.folder(name);
        fs::create_dir(&target)?;
        let all: Vec<_> = self.archive.entries().iter().collect();
        let options = ExtractOptions {
            convert,
            policy: ConflictPolicy::Overwrite,
            ..ExtractOptions::default()
        };
        let report = extract(&self.archive, &all, &target, &options);
        assert!(report.is_complete(), "extraction complete: {report:?}");
        Ok(target)
    }

    fn convert(
        &self,
        input: &Path,
        name: &str,
        filter: Option<&Filter>,
        copy_other: bool,
    ) -> Result<ExtractionReport, Box<dyn Error>> {
        let source = FolderSource::scan(input, None)?;
        let options = ExtractOptions {
            policy: ConflictPolicy::Overwrite,
            ..ExtractOptions::default()
        };
        let selection = source.select(filter, &Exclusion::default());
        Ok(convert_folder(
            &source,
            &selection,
            copy_other,
            &self.folder(name),
            &options,
        )?)
    }
}

fn tree(root: &Path) -> Result<BTreeMap<String, Vec<u8>>, Box<dyn Error>> {
    let mut found = BTreeMap::new();
    let mut folders = vec![root.to_path_buf()];
    while let Some(folder) = folders.pop() {
        for entry in fs::read_dir(folder)? {
            let path = entry?.path();
            if path.is_dir() {
                folders.push(path);
            } else {
                let name = path
                    .strip_prefix(root)?
                    .to_string_lossy()
                    .replace('\\', "/");
                found.insert(name, fs::read(&path)?);
            }
        }
    }
    Ok(found)
}

#[test]
fn files_without_a_conversion_are_counted_and_copied_only_on_request() -> TestResult {
    let fixture = Fixture::new()?;
    let raw = fixture.extract("raw", false)?;

    let report = fixture.convert(&raw, "converted-only", None, false)?;

    assert!(report.is_complete(), "complete: {report:?}");
    let written = tree(&fixture.folder("converted-only"))?;
    assert!(!written.contains_key("Data/readme.bin"), "not copied");
    assert!(
        !written.contains_key("Data/Ships/text.xml"),
        "text XML is not converted"
    );
    assert!(
        written.contains_key("Data/Ships/gladius.xml"),
        "CryXml converted"
    );
    assert_eq!(
        report.not_converted(),
        2,
        "readme.bin and text.xml: {report:?}"
    );
    Ok(())
}

#[test]
fn a_texture_header_without_its_parts_is_reported_and_kept_raw() -> TestResult {
    let fixture = Fixture::new()?;
    let raw = fixture.extract("raw", false)?;
    for part in 1..=3 {
        fs::remove_file(raw.join(format!("Data/Textures/rock_diff.dds.{part}")))?;
    }

    let report = fixture.convert(&raw, "out", None, false)?;

    let unconverted: Vec<_> = report
        .unconverted()
        .iter()
        .map(|f| f.path().to_owned())
        .collect();
    assert_eq!(unconverted, [TEXTURE], "the header is named: {report:?}");
    assert!(
        fixture
            .folder("out")
            .join("Data/Textures/rock_diff.dds")
            .is_file(),
        "the header is not dropped"
    );
    Ok(())
}

#[test]
fn a_filter_selects_the_files_by_their_relative_path() -> TestResult {
    let fixture = Fixture::new()?;
    let raw = fixture.extract("raw", false)?;
    let filter = Filter::new("Data\\Ships\\*");

    let report = fixture.convert(&raw, "out", Some(&filter), true)?;

    assert!(report.is_complete(), "complete: {report:?}");
    let names: Vec<_> = tree(&fixture.folder("out"))?.into_keys().collect();
    assert_eq!(
        names,
        [
            "Data/Ships/gladius.xml",
            "Data/Ships/gladius.xml.p4kx-origin.jsonl",
            "Data/Ships/text.xml",
            "Data/Ships/text.xml.p4kx-origin.jsonl",
        ]
    );
    Ok(())
}

impl Fixture {
    fn convert_excluding(
        &self,
        input: &Path,
        exclude: &[&str],
    ) -> Result<ExtractionReport, Box<dyn Error>> {
        let source = FolderSource::scan(input, None)?;
        let options = ExtractOptions {
            policy: ConflictPolicy::Overwrite,
            ..ExtractOptions::default()
        };
        let selection = source.select(None, &Exclusion::parse(exclude)?);
        Ok(convert_folder(
            &source,
            &selection,
            false,
            &self.folder("out"),
            &options,
        )?)
    }
}

#[test]
fn an_excluded_texture_header_leaves_out_its_parts() -> TestResult {
    let fixture = Fixture::new()?;
    let raw = fixture.extract("raw", false)?;

    let report = fixture.convert_excluding(&raw, &["rock_diff.dds", "*.wem"])?;

    assert!(report.is_complete(), "complete: {report:?}");
    let names: Vec<_> = tree(&fixture.folder("out"))?.into_keys().collect();
    assert!(
        names.iter().all(|name| !name.contains("rock_diff")),
        "no texture file, stray parts included: {names:?}"
    );
    assert!(
        names.iter().all(|name| !name.contains("5.wav")),
        "the sound is left out: {names:?}"
    );
    assert!(
        names.contains(&"Data/Ships/gladius.xml".to_owned()),
        "{names:?}"
    );
    Ok(())
}

#[test]
fn an_excluded_part_is_not_written_but_still_builds_its_texture() -> TestResult {
    let fixture = Fixture::new()?;
    let raw = fixture.extract("raw", false)?;
    let source = FolderSource::scan(&raw, None)?;
    let exclusion = Exclusion::parse(["rock_diff.dds.2"])?;

    let counts = source.select(None, &exclusion);
    let report = fixture.convert_excluding(&raw, &["rock_diff.dds.2"])?;

    assert_eq!(counts.excluded_parts(), 1, "{counts:?}");
    assert_eq!(
        counts.len() + 1,
        counts.matched(),
        "one file left out: {counts:?}"
    );
    assert!(report.is_complete(), "complete: {report:?}");
    let names: Vec<_> = tree(&fixture.folder("out"))?.into_keys().collect();
    assert!(
        names.iter().any(|name| name.ends_with("rock_diff.png")),
        "the texture is built from all its parts: {names:?}"
    );
    assert!(
        names.iter().all(|name| !name.ends_with(".dds.2")),
        "the part is not written: {names:?}"
    );
    Ok(())
}

#[test]
fn overlapping_input_and_output_folders_are_refused_and_nothing_is_written() -> TestResult {
    let fixture = Fixture::new()?;
    let raw = fixture.extract("raw", false)?;
    let before = tree(&raw)?;
    let source = FolderSource::scan(&raw, None)?;
    let options = ExtractOptions::default();

    for target in [
        raw.clone(),
        raw.join("inside"),
        fixture.dir.path().to_path_buf(),
    ] {
        let all = source.select(None, &Exclusion::default());
        let refused = convert_folder(&source, &all, true, &target, &options);
        assert!(
            matches!(refused, Err(FolderError::Overlap { .. })),
            "{}: {refused:?}",
            target.display()
        );
    }
    assert_eq!(tree(&raw)?, before, "the input folder is untouched");
    assert!(!raw.join("inside").exists(), "no folder was created");
    Ok(())
}

#[cfg(unix)]
fn link(original: &Path, link: &Path) -> bool {
    std::os::unix::fs::symlink(original, link).is_ok()
}

#[cfg(windows)]
fn link(original: &Path, link: &Path) -> bool {
    std::os::windows::fs::symlink_dir(original, link).is_ok()
        || std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(link)
            .arg(original)
            .output()
            .is_ok_and(|output| output.status.success())
}

#[test]
fn a_link_in_the_input_folder_is_not_followed() -> TestResult {
    let fixture = Fixture::new()?;
    let raw = fixture.extract("raw", false)?;
    let outside = fixture.folder("outside");
    fs::create_dir(&outside)?;
    fs::write(outside.join("secret.xml"), ship())?;
    assert!(
        link(&outside, &raw.join("Data").join("linked")),
        "a link can be created"
    );

    let report = fixture.convert(&raw, "out", None, true)?;

    let written = tree(&fixture.folder("out"))?;
    assert!(
        written
            .keys()
            .all(|name| !name.contains("secret") && !name.contains("linked")),
        "nothing behind the link: {:?}",
        written.keys().collect::<Vec<_>>()
    );
    let failure = report
        .failures()
        .iter()
        .find(|f| f.path() == "Data\\linked");
    assert!(
        matches!(
            failure.map(EntryFailure::error),
            Some(p4kx_extraction::ExtractionError::Folder(
                FolderFileError::Link
            ))
        ),
        "the link is reported: {report:?}"
    );
    Ok(())
}

#[test]
fn a_path_that_is_no_folder_is_refused() -> TestResult {
    let fixture = Fixture::new()?;
    let file = fixture.folder("Data.p4k");

    assert!(matches!(
        FolderSource::scan(&file, None),
        Err(FolderError::NotAFolder(_))
    ));
    Ok(())
}
