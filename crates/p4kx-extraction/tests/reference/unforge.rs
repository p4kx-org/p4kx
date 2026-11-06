// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Tree comparison of the `CryXml` unforge converted with what P4KX reads.

use std::io::Read;
use std::path::Path;
use std::time::Instant;

use p4kx_extraction::cryxml::{self, Xml};
use p4kx_extraction::{Archive, Entry};
use p4kx_testkit::{real_archive_key, real_archive_path, reference_unforge_path};
use rayon::prelude::*;

use super::TestResult;
use super::reference_error::Result as ReferenceResult;
use super::reference_manifest::{Expected, Tool};
use super::reference_run::{self, Dataset};
use super::tally::{Compared, Tally};
use super::xml_tree::{XmlNode, compare};

const MAX_XML_SIZE: u64 = 64 << 20;

fn our_tree(archive: &Archive, entry: &Entry) -> Result<XmlNode, String> {
    if entry.uncompressed_size() > MAX_XML_SIZE {
        return Err("larger than the comparison reads".to_owned());
    }
    let mut bytes = Vec::new();
    let mut reader = archive
        .open_entry(entry)
        .map_err(|error| error.to_string())?;
    reader
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    match cryxml::read(&bytes).map_err(|error| error.to_string())? {
        Xml::CryXml(document) => {
            XmlNode::from_cryxml(document.root()).map_err(|_| "tree too deep".to_owned())
        }
        Xml::Text(text) => {
            let text = std::str::from_utf8(text).map_err(|error| error.to_string())?;
            let parsed = roxmltree::Document::parse(text).map_err(|error| error.to_string())?;
            XmlNode::from_roxml(parsed.root_element()).map_err(|_| "tree too deep".to_owned())
        }
    }
}

fn reference_tree(bytes: &[u8]) -> Result<XmlNode, String> {
    let text = std::str::from_utf8(bytes).map_err(|error| error.to_string())?;
    let parsed = roxmltree::Document::parse(text).map_err(|error| error.to_string())?;
    XmlNode::from_roxml(parsed.root_element()).map_err(|_| "tree too deep".to_owned())
}

fn reference(dataset: &Dataset, expected: &Expected) -> Result<XmlNode, String> {
    let target = expected
        .targets
        .first()
        .ok_or("selected source has no target")?;
    let bytes = dataset
        .read(&target.path)
        .map_err(|error| error.to_string())?;
    reference_tree(&bytes)
}
fn one(archive: &Archive, dataset: &Dataset, expected: &Expected) -> Tally {
    let mut tally = Tally::default();
    let source = dataset.source.entry(archive, &expected.identity);
    let reference = reference(dataset, expected);
    let compared = match source {
        Err(error) => Compared::Different(error.to_string()),
        Ok(entry) => match (our_tree(archive, entry), reference) {
            (Ok(ours), Ok(reference)) => compare(&ours, &reference),
            (Err(error), _) | (_, Err(error)) => Compared::Different(error),
        },
    };
    tally.record("selected XML source", compared);
    tally
}

pub(crate) fn verify(archive: &Archive, root: &Path) -> ReferenceResult<Tally> {
    let dataset = Dataset::load(root, archive, Tool::Unforge)?;
    let tally = dataset
        .manifest
        .expected
        .par_iter()
        .map(|expected| one(archive, &dataset, expected))
        .reduce(Tally::default, Tally::merge);
    dataset.finish(archive, &tally, "unforge tree comparison")?;
    Ok(tally)
}

#[test]
#[ignore = "real Archive and reference output: set P4KX_DATA_P4K and P4KX_REF_UNFORGE and pass --ignored"]
fn every_cryxml_tree_unforge_converted_is_identical() -> TestResult {
    reference_run::optional(reference_unforge_path(), "P4KX_REF_UNFORGE", |root| {
        let archive = Archive::open_with_key(real_archive_path()?, Some(real_archive_key()?))?;
        let started = Instant::now();
        verify(&archive, root)?;
        super::report(&format!("unforge run took {:?}", started.elapsed()));
        Ok(())
    })
}
