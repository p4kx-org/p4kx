// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::OnceLock;

use p4kx_extraction::{ProvenanceMode, SourceUses, SourceValue, SourcedValue};

use super::{LoadoutPort, PortType};
use crate::guid_text::CatalogGuid;
use crate::loadout::alignment::LoadoutAlignment;

#[derive(Clone)]
struct UnobservedPortFields {
    size: SourcedValue<u32>,
    words: SourcedValue<Vec<String>>,
    name_key: SourcedValue<String>,
    item_guid: SourcedValue<CatalogGuid>,
    visibility: SourcedValue<&'static str>,
    types: SourcedValue<Vec<PortType>>,
}

impl UnobservedPortFields {
    fn with_mode(mode: ProvenanceMode) -> Self {
        static MARKERS: OnceLock<UnobservedPortFields> = OnceLock::new();
        static FULL: OnceLock<UnobservedPortFields> = OnceLock::new();
        match mode {
            ProvenanceMode::MarkersOnly => MARKERS.get_or_init(|| Self::new(mode)).clone(),
            ProvenanceMode::Full => FULL.get_or_init(|| Self::new(mode)).clone(),
            _ => Self::new(mode),
        }
    }

    fn new(mode: ProvenanceMode) -> Self {
        let uses = SourceUses::with_provenance_mode(mode);
        Self {
            size: uses.value(SourceValue::Absent, &[], false),
            words: uses.value(SourceValue::Absent, &[], false),
            name_key: uses.value(SourceValue::Absent, &[], false),
            item_guid: uses.value(SourceValue::Absent, &[], false),
            visibility: uses.value(SourceValue::Absent, &[], false),
            types: uses.value(SourceValue::Absent, &[], false),
        }
    }
}

impl LoadoutPort {
    pub(crate) fn named(name: SourcedValue<String>) -> Self {
        let UnobservedPortFields {
            size,
            words,
            name_key,
            item_guid,
            visibility,
            types,
        } = UnobservedPortFields::with_mode(name.origin().provenance_mode());
        Self {
            name,
            name_proof: None,
            attachment_port_inventory_complete: false,
            source: "",
            min_size: size.clone(),
            max_size: size,
            types,
            flags: words.clone(),
            port_tags: words.clone(),
            required_tags: words,
            display_name_key: name_key.clone(),
            visibility,
            item_guid,
            item_class_name: name_key,
            docked_vehicle: false,
            unresolved: None,
            source_constraints: LoadoutAlignment::default(),
            ports: Vec::new(),
        }
    }
}
