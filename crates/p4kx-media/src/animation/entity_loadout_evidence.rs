// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::ops::ControlFlow;
use std::sync::Mutex;

use p4kx_assembly::{
    EntityRig, LoadoutClassSelection, LoadoutFileObservation, LoadoutPath, LoadoutReadEvent,
    LoadoutReadObserver, LoadoutSelection, LoadoutTargetGuid, LoadoutXmlDecoding,
    LoadoutXmlRecordSelector, XmlLoadoutClassSite, XmlLoadoutPathSite,
};
use p4kx_extraction::{DataForge, EntryOrigin, InputScope, SourceLocation, SourceUseRole};

use crate::origin_capture::{OriginCapture, OriginSite};
use crate::origin_contributions::OriginContributions;
use crate::origin_input::MediaOriginInput;
use crate::{MediaError, MediaIssue, MediaWriter, origin_error};

#[path = "entity_loadout_attempt_issue.rs"]
mod attempts;

use self::attempts::AttemptIssue;

pub(super) struct ObservedLoadoutIssue {
    pub(super) issue: MediaIssue,
    pub(super) origin: OriginContributions,
}

pub(crate) struct EntityLoadoutEvidence<'forge> {
    forge: &'forge DataForge<'forge>,
    sites: Mutex<Sites>,
}

#[derive(Default)]
struct Sites {
    paths: HashMap<LoadoutSelection, Site>,
    classes: HashMap<LoadoutClassSelection, Site>,
    xml_roots: HashMap<LoadoutSelection, [u64; 2]>,
    attempts: Vec<AttemptIssue>,
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

impl<'forge> EntityLoadoutEvidence<'forge> {
    pub(crate) fn new(forge: &'forge DataForge<'forge>) -> Self {
        Self {
            forge,
            sites: Mutex::default(),
        }
    }

    pub(super) fn issues(
        &self,
        dataforge: &MediaOriginInput,
        writer: &MediaWriter<'_>,
    ) -> Result<Vec<ObservedLoadoutIssue>, MediaError> {
        let sites = self.sites.lock().map_err(|_| origin_error::foreign())?;
        sites
            .attempts
            .iter()
            .map(|attempt| {
                writer.checkpoint()?;
                let mut capture = writer.capture_origins();
                if let Some(selection) = attempt.request.selection() {
                    if let Some(site) = sites.paths.get(selection) {
                        site.add(&mut capture, writer)?;
                    } else if let Some(indices) = sites.xml_roots.get(selection) {
                        capture.add(
                            dataforge,
                            InputScope::Payload,
                            OriginSite::new("dataforgeValue", indices, "$value"),
                            SourceUseRole::Selector,
                            writer,
                        )?;
                    } else {
                        capture.include_original(&OriginContributions::unknown(
                            &writer.origins.owner,
                        ))?;
                    }
                } else {
                    capture
                        .include_original(&OriginContributions::unknown(&writer.origins.owner))?;
                }
                attempt.add_original(&mut capture, writer)?;
                Ok(ObservedLoadoutIssue {
                    issue: attempt.issue(),
                    origin: capture.finish(),
                })
            })
            .collect()
    }

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

    pub(crate) fn position_route(
        &self,
        rig: &EntityRig,
        dataforge: &MediaOriginInput,
        writer: &MediaWriter<'_>,
    ) -> Result<OriginContributions, MediaError> {
        let mut capture = writer.capture_origins();
        let selectors = rig.port_selectors();
        let paths = rig.port_name_paths();
        if selectors.len() != paths.len() {
            return Err(MediaError::Contract(
                "animation port route lost a physical selector".into(),
            ));
        }
        for (selector, path) in selectors.iter().zip(paths) {
            writer.checkpoint()?;
            if selector.is_none()
                && let Some(path) = path
                && self.path(path, &mut capture, writer)?
            {
                continue;
            }
            record_location(&mut capture, dataforge, selector.as_ref(), writer)?;
        }
        Ok(capture.finish())
    }

    pub(crate) fn class(
        &self,
        target: &LoadoutTargetGuid,
        input: &MediaOriginInput,
        writer: &MediaWriter<'_>,
    ) -> Result<OriginContributions, MediaError> {
        let mut capture = writer.capture_origins();
        let Some(route) = target.class_xml_use() else {
            return Ok(capture.finish());
        };
        let sites = self.sites.lock().map_err(|_| origin_error::foreign())?;
        let root = route.original_root();
        if root.read_outcome().is_ok()
            && root.source().entry_origin().is_some()
            && let Some(indices) = route
                .caller()
                .selection()
                .and_then(|selection| sites.xml_roots.get(selection))
        {
            capture.add(
                input,
                InputScope::Payload,
                OriginSite::new("dataforgeValue", indices, "$value"),
                SourceUseRole::Selector,
                writer,
            )?;
        }
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

pub(crate) fn record_location(
    capture: &mut OriginCapture,
    dataforge: &MediaOriginInput,
    location: Option<&SourceLocation>,
    writer: &MediaWriter<'_>,
) -> Result<(), MediaError> {
    let Some(SourceLocation::DataForge {
        record_index,
        field,
    }) = location
    else {
        return capture.include(&OriginContributions::unknown(&writer.origins.owner), writer);
    };
    capture.add(
        dataforge,
        InputScope::Payload,
        OriginSite::new("dataforge-record", &[u64::from(*record_index)], field),
        SourceUseRole::Selector,
        writer,
    )
}

impl LoadoutReadObserver for EntityLoadoutEvidence<'_> {
    fn observe(&self, event: LoadoutReadEvent<'_, '_>) -> ControlFlow<()> {
        let Some(attempt) = AttemptIssue::from_event(&event) else {
            return ControlFlow::Continue(());
        };
        let Ok(mut sites) = self.sites.lock() else {
            return ControlFlow::Break(());
        };
        if sites.attempts.try_reserve(1).is_err() {
            return ControlFlow::Break(());
        }
        sites.attempts.push(attempt);
        ControlFlow::Continue(())
    }

    fn observes_port_names(&self) -> bool {
        true
    }

    fn observes_xml_record_selectors(&self) -> bool {
        true
    }

    fn xml_record_selector(&self, site: &LoadoutXmlRecordSelector<'_, '_>) -> ControlFlow<()> {
        if !site.record().is_from(self.forge) {
            return ControlFlow::Continue(());
        }
        let Ok(mut sites) = self.sites.lock() else {
            return ControlFlow::Break(());
        };
        if sites.xml_roots.try_reserve(1).is_err() {
            return ControlFlow::Break(());
        }
        sites.xml_roots.insert(
            site.selection().clone(),
            [u64::from(site.record().index()), site.ordinal() as u64],
        );
        ControlFlow::Continue(())
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
