// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::{Bounds, Level, children, entxml, hcd};
use crate::CatalogError;
use crate::catalog_read::original::{EntryRead, EntryXml, XmlNodeSource};
use crate::export_ledger::ExportLedger;
use crate::starmap_containers::instance::ContainerFile;
use crate::starmap_containers::jump_gate::{GateFiles, JumpGate};
use crate::starmap_index::Index;
use crate::starmap_xml::XmlError;

pub(super) struct ParsedRoot {
    pub(super) root: Level,
    pub(super) pending_external: bool,
    pub(super) file: ContainerFile,
    pub(super) entities: entxml::EntXmlIndex,
    pub(super) gate: Option<Box<JumpGate>>,
    pub(super) gate_proof: Option<XmlNodeSource>,
}

pub(super) fn read(
    read: &EntryRead,
    bounds: &Bounds<'_>,
    files: &entxml::EntXmlFiles<'_, '_>,
    index: &Index<'_>,
    ledger: &mut ExportLedger,
) -> Result<Result<ParsedRoot, XmlError>, CatalogError> {
    let xml = match EntryXml::parse_starmap_main(read, files.capture)? {
        Ok(xml) => xml,
        Err(error) => return Ok(Err(error)),
    };
    let mut hcd = hcd::HcdReader::new(
        files.archive,
        files.outer,
        files.container,
        files.capture,
        files.cancel,
    );
    let mut reader = children::ChildReader::new(bounds, &xml, &mut hcd, index, ledger);
    let root = reader.level(xml.root())?;
    let pending_external = reader.pending_external;
    let guids = reader.entity_guids;
    let entities = files.scan(&guids, &xml, ledger)?;
    let (gate, gate_proof) = if bounds.scope.reads_jump_endpoints() {
        let readers = GateFiles {
            archive: files.archive,
            outer: files.outer,
            container: files.container,
            capture: files.capture,
            cancel: files.cancel,
        };
        (
            Some(Box::new(readers.read(&xml, ledger)?)),
            Some(xml.node_source(xml.root())?),
        )
    } else {
        (None, None)
    };
    let mut file = ContainerFile::from_xml(&xml, files.capture)?;
    if bounds.scope.reads_location_physics() {
        file.location_physics = crate::location_physics::read(
            read,
            &xml,
            files,
            &bounds.location_grid_paths,
            bounds.directory.as_ref(),
        )?;
    }
    Ok(Ok(ParsedRoot {
        root,
        pending_external,
        file,
        entities,
        gate,
        gate_proof,
    }))
}
