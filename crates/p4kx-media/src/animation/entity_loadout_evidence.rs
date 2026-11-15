// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::ops::ControlFlow;
use std::sync::Mutex;

use p4kx_assembly::{
    LoadoutClassSelection, LoadoutFileObservation, LoadoutPath, LoadoutReadEvent,
    LoadoutReadObserver, LoadoutSelection, LoadoutXmlDecoding, XmlLoadoutClassSite,
    XmlLoadoutPathSite,
};
use p4kx_extraction::{EntryOrigin, InputScope, SourceUseRole};

use crate::origin_capture::{OriginCapture, OriginSite};
use crate::origin_contributions::OriginContributions;
use crate::{MediaError, MediaWriter, origin_error};

#[derive(Default)]
pub(crate) struct EntityLoadoutEvidence {
    sites: Mutex<Sites>,
}

#[derive(Default)]
struct Sites {
    paths: HashMap<LoadoutSelection, Site>,
    classes: HashMap<LoadoutClassSelection, Site>,
}

struct Site {
    input: EntryOrigin,
    decoding: LoadoutXmlDecoding,
    indices: [u64; 2],
    attribute: String,
}

impl Site {
    fn new(
        observation: &LoadoutFileObservation,
        decoding: LoadoutXmlDecoding,
        node: u32,
        attribute: u64,
        name: &str,
    ) -> Option<Self> {
        observation.read_outcome().ok()?;
        let input = observation.source().entry_origin()?.clone();
        Some(Self {
            input,
            decoding,
            indices: [u64::from(node), attribute],
            attribute: name.to_owned(),
        })
    }

    fn add(&self, capture: &mut OriginCapture, writer: &MediaWriter<'_>) -> Result<(), MediaError> {
        let namespace = match self.decoding {
            LoadoutXmlDecoding::Text { .. } => "loadout-xml-attribute",
            LoadoutXmlDecoding::CryXml { .. } => "loadout-cryxml-attribute",
            _ => {
                return Err(MediaError::Contract(
                    "unknown Loadout attribute coordinate".into(),
                ));
            }
        };
        let input = writer.origins.observer()?.original(&self.input)?;
        capture.add(
            &input,
            InputScope::Payload,
            OriginSite::new(namespace, &self.indices, &self.attribute),
            SourceUseRole::Selector,
            writer,
        )
    }
}

impl EntityLoadoutEvidence {
    pub(crate) fn path(
        &self,
        path: &LoadoutPath,
        capture: &mut OriginCapture,
        writer: &MediaWriter<'_>,
    ) -> Result<bool, MediaError> {
        let sites = self.sites.lock().map_err(|_| origin_error::foreign())?;
        let site = path
            .selection()
            .and_then(|selection| sites.paths.get(selection));
        if let Some(site) = site {
            site.add(capture, writer)?;
        }
        Ok(site.is_some())
    }

    pub(crate) fn class(
        &self,
        target: &p4kx_assembly::LoadoutTargetGuid,
        writer: &MediaWriter<'_>,
    ) -> Result<OriginContributions, MediaError> {
        let mut capture = writer.capture_origins();
        let Some(route) = target.class_xml_use() else {
            return Ok(capture.finish());
        };
        let sites = self.sites.lock().map_err(|_| origin_error::foreign())?;
        let class = target
            .class_selection()
            .and_then(|selection| sites.classes.get(selection));
        if let Some(site) = class {
            site.add(&mut capture, writer)?;
        }
        drop(sites);
        for (_, _, request) in route.includes() {
            writer.checkpoint()?;
            self.path(request, &mut capture, writer)?;
        }
        capture.include_original(&OriginContributions::unknown(&writer.origins.owner))?;
        Ok(capture.finish())
    }
}

impl LoadoutReadObserver for EntityLoadoutEvidence {
    fn observe(&self, _event: LoadoutReadEvent<'_, '_>) -> ControlFlow<()> {
        ControlFlow::Continue(())
    }

    fn observes_port_names(&self) -> bool {
        true
    }

    fn xml_path(
        &self,
        selection: &LoadoutSelection,
        site: &XmlLoadoutPathSite<'_>,
    ) -> ControlFlow<()> {
        let Ok(mut sites) = self.sites.lock() else {
            return ControlFlow::Break(());
        };
        if let Some(site) = Site::new(
            site.observation(),
            site.decoding(),
            site.node_index(),
            site.attribute_index(),
            site.attribute_name(),
        ) {
            if sites.paths.try_reserve(1).is_err() {
                return ControlFlow::Break(());
            }
            sites.paths.insert(selection.clone(), site);
        }
        ControlFlow::Continue(())
    }

    fn xml_class(
        &self,
        selection: &LoadoutClassSelection,
        site: &XmlLoadoutClassSite<'_>,
    ) -> ControlFlow<()> {
        let Ok(mut sites) = self.sites.lock() else {
            return ControlFlow::Break(());
        };
        if let Some(site) = Site::new(
            site.observation(),
            site.decoding(),
            site.node_index(),
            site.attribute_index(),
            site.attribute_name(),
        ) {
            if sites.classes.try_reserve(1).is_err() {
                return ControlFlow::Break(());
            }
            sites.classes.insert(selection.clone(), site);
        }
        ControlFlow::Continue(())
    }
}
