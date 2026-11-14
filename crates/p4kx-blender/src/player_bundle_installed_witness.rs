// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::io;

use p4kx_assembly::BodyVariant;
use p4kx_extraction::{
    Archive, CancelToken, EntryOrigin, SourceEncryption, SourceInventory, SourceLocation,
    SourceUseDescription, SourceUseRole,
};
use serde_json::Value;

use crate::player_bundle_rows;
use crate::player_bundle_wire::Row;
use crate::soc_scene_xml::XmlDocuments;
use crate::{PlayerDocument, PlayerDocumentSink};

use super::{PreparedCapture, prepare_observed};

#[path = "player_bundle_witness_actor.rs"]
mod actor;
#[path = "player_bundle_witness_xml.rs"]
mod xml;

type Check<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[derive(Default)]
struct Originals(BTreeMap<u64, xml::Document>);

#[derive(Debug)]
struct Physical {
    index: usize,
    encrypted: Option<bool>,
}

struct Sources<'capture> {
    sources: BTreeMap<String, usize>,
    uses: BTreeMap<&'capture str, &'capture SourceUseDescription>,
}

#[test]
#[ignore = "selected Player bundle sources in installed Archive; set P4KX_DATA_P4K"]
fn installed_player_bundle_matches_selected_original_sources() -> Check {
    let path = p4kx_testkit::real_archive_path()?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::real_archive_key()?))
        .map_err(|_| "cannot open selected installed Archive")?;
    let mut documents = Originals::default();
    let mut original_actor = None;
    let prepared = prepare_observed(
        &archive,
        &[BodyVariant::Male, BodyVariant::Female],
        &CancelToken::new(),
        XmlDocuments::stream_to(&mut documents),
        |data, classes| original_actor = Some(actor::observe(data, classes)),
    )
    .map_err(|_| "cannot capture selected Player sources")?;
    let capture = PreparedCapture::into_capture(prepared);
    let actor = original_actor.ok_or("no actor source observation")??;
    let sources = Sources::new(&capture.source_inventory)?;
    let mut coverage = xml::Coverage::default();
    let mut candidates = 0;
    player_bundle_rows::write(&capture, |row| {
        match row {
            Row::ActorRigCandidate { value, .. } => {
                actor.verify(candidates, &serde_json::to_value(value)?, &sources)?;
                candidates += 1;
            }
            Row::ActorRigCandidates { state, .. }
                if *state != p4kx_extraction::SourceValueState::Present =>
            {
                return Err(io::Error::other("actor geometry inventory unavailable"));
            }
            _ => coverage.verify(row, &documents.0, &sources)?,
        }
        Ok(())
    })?;
    if candidates == 0 || candidates != actor.len() {
        return Err("actor geometry candidate coverage differs".into());
    }
    coverage.finish(&documents.0)?;
    Ok(())
}

impl PlayerDocumentSink for Originals {
    fn document(&mut self, document: PlayerDocument<'_>) -> io::Result<()> {
        if let Some(expected) = xml::Document::observe(&document)?
            && self.0.insert(document.index as u64, expected).is_some()
        {
            return Err(io::Error::other("duplicate source document"));
        }
        Ok(())
    }
}

impl Physical {
    fn of(original: &EntryOrigin) -> Self {
        let encrypted = match original.encryption() {
            SourceEncryption::Encrypted => Some(true),
            SourceEncryption::Unencrypted => Some(false),
            _ => None,
        };
        Self {
            index: original.entry().index(),
            encrypted,
        }
    }
}

impl<'capture> Sources<'capture> {
    fn new(inventory: &'capture SourceInventory) -> io::Result<Self> {
        let mut sources = BTreeMap::new();
        for source in inventory.sources() {
            let wire = serde_json::to_value(source)?;
            if let Some(index) = wire["entryIndex"].as_u64() {
                let index = usize::try_from(index).map_err(io::Error::other)?;
                sources.insert(source.id().as_str().to_owned(), index);
            }
        }
        let uses = inventory
            .uses()
            .iter()
            .map(|row| (row.id().as_str(), row))
            .collect();
        Ok(Self { sources, uses })
    }

    fn field(
        &self,
        field: &Value,
        expected: &Physical,
        coordinate: &SourceLocation,
    ) -> io::Result<()> {
        let references = field["sourceUsesRefs"]
            .as_array()
            .ok_or_else(|| io::Error::other("sourced field has no contributor list"))?;
        let bound = references.iter().any(|reference| {
            reference
                .as_str()
                .and_then(|id| self.uses.get(id))
                .is_some_and(|used| {
                    self.sources.get(used.source().as_str()) == Some(&expected.index)
                        && used.location() == coordinate
                        && matches!(
                            used.role(),
                            SourceUseRole::Payload | SourceUseRole::Selector
                        )
                })
        });
        if !bound || field["containsEncryptedSource"].as_bool() != expected.encrypted {
            return Err(io::Error::other(
                "field physical coordinate or encryption differs",
            ));
        }
        Ok(())
    }
}
