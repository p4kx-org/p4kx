// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::env;
use std::io::{self, BufReader, Read};
use std::path::PathBuf;

use p4kx_extraction::Checksum;
use sha2::{Digest, Sha256};

use super::artifact;
use super::sections::{SectionReader, SectionReport};
use super::support::{self, TestResult};

#[path = "real_archive_catalog_replay_bindings.rs"]
pub(crate) mod bindings;
#[path = "real_archive_catalog_replay_cells.rs"]
mod cells;
#[path = "real_archive_catalog_replay_inventory.rs"]
mod inventory;

#[cfg(test)]
#[path = "real_archive_catalog_replay_tests.rs"]
mod tests;

use bindings::{Binding, IndexedPass, Receipt};
use cells::Rows;

#[test]
#[ignore = "completed private catalog measurement: set P4KX_CATALOG_MEASUREMENT_DIR"]
fn replay_completed_catalog_provenance_and_sections() -> TestResult {
    let directory = env::var_os("P4KX_CATALOG_MEASUREMENT_DIR")
        .map(PathBuf::from)
        .ok_or("P4KX_CATALOG_MEASUREMENT_DIR is required")?;
    let binding = Receipt::read(&directory)?.binding()?;
    let first = artifact::read_completed(&directory, |input| index(input))?;
    support::print_line(&format!(
        "catalog replay indexed decodedBytes={} physicalSources={} sourceUses={}; verifying cells",
        first.sections.decoded_bytes, first.sections.physical_sources, first.sections.source_uses,
    ));
    let second = artifact::read_completed(&directory, |input| verify(input, &first, &binding))?;
    support::print_line(&format!(
        "catalog replay decodedBytes={} decodedSha256={} physicalSources={} sourceUses={} variantSourceUseSets={} rows={} sections={}",
        first.sections.decoded_bytes,
        Checksum::Sha256(first.digest).hex(),
        first.sections.physical_sources,
        first.sections.source_uses,
        first.pass.inventory.variant_set_count(),
        serde_json::to_string(&second)?,
        serde_json::to_string(&first.sections.sections)?
    ));
    Ok(())
}

struct Indexed {
    digest: [u8; 32],
    pass: IndexedPass,
    sections: SectionReport,
}

fn index(input: impl Read) -> TestResult<Indexed> {
    let mut input = DigestReader::new(input);
    let mut sections = SectionReader::new(&mut input);
    let pass = IndexedPass::read(BufReader::new(&mut sections))?;
    let sections = sections.finish()?;
    if pass.inventory.counts() != (sections.physical_sources, sections.source_uses) {
        return Err("streamed inventory counts disagree with section bytes".into());
    }
    Ok(Indexed {
        digest: input.digest.finalize().into(),
        pass,
        sections,
    })
}

fn verify(input: impl Read, indexed: &Indexed, binding: &Binding) -> TestResult<Rows> {
    binding.check(&indexed.pass)?;
    let mut input = DigestReader::new(input);
    let rows = cells::verify(BufReader::new(&mut input), &indexed.pass.inventory)?;
    let digest: [u8; 32] = input.digest.finalize().into();
    if digest != indexed.digest {
        return Err("catalog bytes changed between validation passes".into());
    }
    Ok(rows)
}

struct DigestReader<R> {
    input: R,
    digest: Sha256,
}

impl<R> DigestReader<R> {
    fn new(input: R) -> Self {
        Self {
            input,
            digest: Sha256::new(),
        }
    }
}

impl<R: Read> Read for DigestReader<R> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        let count = self.input.read(bytes)?;
        let decoded = bytes
            .get(..count)
            .ok_or_else(|| io::Error::other("decoded read exceeded its buffer"))?;
        self.digest.update(decoded);
        Ok(count)
    }
}
