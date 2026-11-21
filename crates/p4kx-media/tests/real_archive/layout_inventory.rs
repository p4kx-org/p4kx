// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::File;
use std::io;
use std::sync::atomic::AtomicBool;

use p4kx_extraction::{
    Archive, ArchiveDeclarationState, ArchiveLayout, ArchiveLayoutInfo, ArchiveWatch, MetadataCost,
    MetadataSection,
};

use super::layout_footer::Footer;
use super::layout_records;
use super::{TestResult, privacy};

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn installed_v1_layout_inventory_matches_independent_source_bytes()
-> Result<(), privacy::VerificationFailure> {
    privacy::redact(verify())
}

fn verify() -> TestResult {
    let path = p4kx_testkit::real_archive_path()?;
    let archive = Archive::open(&path)?;
    let watch = ArchiveWatch::new(&archive);
    watch.check()?;
    let mut file = File::open(&path)?;
    let source_len = file.metadata()?.len();
    let footer = Footer::read(&mut file, source_len)?;
    let view = archive.metadata_view();
    let layout = ArchiveLayoutInfo::of(view.metadata());
    require(
        matches!(layout.layout, ArchiveLayout::Zip64),
        "ZIP64 layout route",
    )?;
    require(
        matches!(layout.declaration_state, ArchiveDeclarationState::Available),
        "available layout declarations",
    )?;
    require(layout.source_length.get() == source_len, "source length")?;
    require(layout.v2.is_none(), "v2 is covered by synthetic fixtures")?;
    for section in [
        MetadataSection::V2BaseDirectory,
        MetadataSection::V2ExtensionDirectory,
        MetadataSection::V2BaseNames,
        MetadataSection::V2ExtensionNames,
        MetadataSection::V2Trailer,
        MetadataSection::V2OpaqueFirst,
        MetadataSection::V2OpaqueSecond,
        MetadataSection::V2Trie,
    ] {
        require(view.region(section)?.is_none(), "v2 sections absent")?;
    }
    footer.verify(layout.zip.as_ref().ok_or("ZIP64 layout")?, source_len)?;
    let mut budget = SampleBudget {
        bytes: 4 << 20,
        work: 4096,
    };
    let mut charge = |cost| budget.charge(cost);
    let cancelled = AtomicBool::new(false);
    let mut access = view.access(&mut charge, &cancelled);
    layout_records::verify(&archive, &mut access, &mut file, &footer)?;
    watch.check()?;
    Ok(())
}

struct SampleBudget {
    bytes: u64,
    work: u64,
}

impl SampleBudget {
    fn charge(&mut self, cost: MetadataCost) -> io::Result<()> {
        self.bytes = self
            .bytes
            .checked_sub(cost.read_bytes())
            .ok_or_else(sample_limit)?;
        self.work = self
            .work
            .checked_sub(cost.work_items())
            .ok_or_else(sample_limit)?;
        Ok(())
    }
}

fn sample_limit() -> io::Error {
    io::Error::other("metadata sample budget exhausted")
}

pub(super) fn require(condition: bool, message: &'static str) -> TestResult {
    if condition {
        Ok(())
    } else {
        Err(message.into())
    }
}
