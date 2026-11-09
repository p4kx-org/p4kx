// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use p4kx_cryxml::{Element, Xml};

use crate::loadout_file_trace::FileTrace;
use crate::merge::merge_by_port;
use crate::resolver_observation::{self, LoadoutReadEvent, Observation};
use crate::spec::{EntrySpec, LoadoutOrigin, LoadoutSpec, TargetSpec};
use crate::{LoadoutDocumentKind, LoadoutError, LoadoutFiles, MAX_LOADOUT_DEPTH};

/// The most XML Loadouts one XML Loadout includes through nested `<Loadouts>`.
pub const MAX_XML_INCLUDE_DEPTH: usize = 8;

/// The most XML Loadout files one resolution of an XML Loadout reads, the Loadout itself and
/// everything it includes; further includes are left out with an issue.
pub const MAX_XML_INCLUDES: usize = 256;

/// One XML Loadout file: the Loadouts it includes and its own entries.
struct XmlLoadout {
    includes: Vec<String>,
    entries: Vec<EntrySpec>,
}

/// An element of text XML or `CryXml`, read alike.
trait Node: Copy {
    fn name(self) -> String;
    fn attribute(self, name: &str) -> Option<String>;
    fn children(self) -> Vec<Self>;
}

impl Node for Element<'_> {
    fn name(self) -> String {
        Element::name(self).to_owned()
    }

    fn attribute(self, name: &str) -> Option<String> {
        Element::attribute(self, name).map(str::to_owned)
    }

    fn children(self) -> Vec<Self> {
        Element::children(self).collect()
    }
}

impl Node for roxmltree::Node<'_, '_> {
    fn name(self) -> String {
        self.tag_name().name().to_owned()
    }

    fn attribute(self, name: &str) -> Option<String> {
        roxmltree::Node::attribute(&self, name).map(str::to_owned)
    }

    fn children(self) -> Vec<Self> {
        roxmltree::Node::children(&self)
            .filter(roxmltree::Node::is_element)
            .collect()
    }
}

/// The entries of an XML Loadout and why parts of it cannot be read.
pub(crate) struct XmlEntries {
    pub(crate) entries: Vec<EntrySpec>,
    pub(crate) issues: Vec<LoadoutError>,
}

/// Reads the XML Loadout at `path` with the Loadouts it includes: the entries of the included
/// Loadouts in order, each overridden by the later ones and by the file's own entries per port.
/// An include that cannot be read is an issue; the rest still loads.
pub(crate) fn load_observed(
    files: &dyn LoadoutFiles,
    path: &str,
    observation: Option<&Observation>,
    trace: &mut FileTrace,
) -> Result<XmlEntries, LoadoutError> {
    let mut loader = Loader {
        observation,
        files,
        trace,
        stack: Vec::new(),
        done: HashMap::new(),
        loads: 0,
        issues: Vec::new(),
    };
    let entries = loader.load(path)?;
    Ok(XmlEntries {
        entries,
        issues: loader.issues,
    })
}

/// Reads one XML Loadout with its includes, each file once.
struct Loader<'l> {
    observation: Option<&'l Observation>,
    files: &'l dyn LoadoutFiles,
    trace: &'l mut FileTrace,
    stack: Vec<String>,
    done: HashMap<String, Result<Vec<EntrySpec>, LoadoutError>>,
    loads: usize,
    issues: Vec<LoadoutError>,
}

impl Loader<'_> {
    fn load(&mut self, path: &str) -> Result<Vec<EntrySpec>, LoadoutError> {
        if self.observation.is_some_and(Observation::stopped) {
            return Err(LoadoutError::ObservationStopped);
        }
        if let Some(done) = self.done.get(&path.to_ascii_lowercase()) {
            return done.clone();
        }
        let failed = |reason: String| LoadoutError::XmlLoadout {
            path: path.to_owned(),
            reason,
        };
        if self
            .stack
            .iter()
            .any(|seen| seen.eq_ignore_ascii_case(path))
        {
            return Err(failed("it includes itself".to_owned()));
        }
        if self.stack.len() >= MAX_XML_INCLUDE_DEPTH {
            resolver_observation::notify(self.observation, LoadoutReadEvent::InventoryIncomplete)?;
            return Err(failed(format!(
                "it is included deeper than {MAX_XML_INCLUDE_DEPTH} Loadouts"
            )));
        }
        self.loads = self.loads.saturating_add(1);
        let loadout = match load_file(self.files, path, self.observation, self.trace) {
            Ok(loadout) => loadout,
            Err(error) => {
                self.done
                    .insert(path.to_ascii_lowercase(), Err(error.clone()));
                return Err(error);
            }
        };
        self.stack.push(path.to_owned());
        let mut entries = self.load_includes(&loadout.includes)?;
        self.stack.pop();
        merge_into(&mut entries, loadout.entries);
        self.done
            .insert(path.to_ascii_lowercase(), Ok(entries.clone()));
        Ok(entries)
    }

    fn load_includes(&mut self, paths: &[String]) -> Result<Vec<EntrySpec>, LoadoutError> {
        let mut entries = Vec::new();
        let mut named = HashSet::new();
        for include in paths {
            if self.observation.is_some_and(Observation::stopped) {
                return Err(LoadoutError::ObservationStopped);
            }
            let key = include.to_ascii_lowercase();
            if !named.insert(key.clone()) {
                continue;
            }
            if self.loads >= MAX_XML_INCLUDES && !self.done.contains_key(&key) {
                resolver_observation::notify(
                    self.observation,
                    LoadoutReadEvent::InventoryIncomplete,
                )?;
                self.issues.push(LoadoutError::XmlLoadout {
                    path: include.clone(),
                    reason: format!(
                        "more than {MAX_XML_INCLUDES} Loadouts are read; the rest are left out"
                    ),
                });
                break;
            }
            match self.load(include) {
                Ok(included) => merge_into(&mut entries, included),
                Err(error) if self.observation.is_some_and(Observation::stopped) => {
                    return Err(error);
                }
                Err(error) => self.issues.push(error),
            }
        }
        Ok(entries)
    }
}

fn load_file(
    files: &dyn LoadoutFiles,
    path: &str,
    observation: Option<&Observation>,
    trace: &mut FileTrace,
) -> Result<XmlLoadout, LoadoutError> {
    let failed = |reason| LoadoutError::XmlLoadout {
        path: path.to_owned(),
        reason,
    };
    let bytes = crate::loadout_file_events::read(
        files,
        path,
        LoadoutDocumentKind::XmlLoadout,
        observation,
        trace,
    )
    .map_err(|error| match error {
        crate::LoadoutFileError::ObservationStopped => LoadoutError::ObservationStopped,
        _ if observation.is_some_and(Observation::stopped) => LoadoutError::ObservationStopped,
        crate::LoadoutFileError::NotFound => LoadoutError::MissingXmlLoadout {
            path: path.to_owned(),
        },
        error => failed(error.to_string()),
    })?;
    let result = parse(&bytes, &LoadoutOrigin::File(Arc::from(path)));
    crate::loadout_file_events::parsed(
        bytes,
        result.as_ref().map(|_| ()).map_err(String::as_str),
        observation,
    )?;
    result.map_err(failed)
}
/// Replaces the entry of each port `overrides` names, ignoring ASCII case, and appends the rest.
fn merge_into(entries: &mut Vec<EntrySpec>, overrides: Vec<EntrySpec>) {
    merge_by_port(entries, overrides, |entry| &entry.port);
}
fn parse(bytes: &[u8], origin: &LoadoutOrigin) -> Result<XmlLoadout, String> {
    match p4kx_cryxml::read(bytes).map_err(|error| error.to_string())? {
        Xml::CryXml(document) => read_root(document.root(), origin),
        Xml::Text(text) => {
            let text = std::str::from_utf8(text).map_err(|error| error.to_string())?;
            let text = text.trim_start_matches('\u{feff}');
            p4kx_cryxml::check_text_depth(text).map_err(|error| error.to_string())?;
            let document = roxmltree::Document::parse(text).map_err(|error| error.to_string())?;
            read_root(document.root_element(), origin)
        }
    }
}

fn read_root<N: Node>(root: N, origin: &LoadoutOrigin) -> Result<XmlLoadout, String> {
    if root.name() != "Loadout" {
        return Err(format!("its root element is {}, not Loadout", root.name()));
    }
    let mut includes = Vec::new();
    let mut entries = Vec::new();
    for child in root.children() {
        match child.name().as_str() {
            "Loadouts" => includes.extend(
                child
                    .children()
                    .into_iter()
                    .filter_map(|include| include.attribute("loadout"))
                    .filter(|path| !path.is_empty()),
            ),
            "Items" => entries.extend(read_items(child, 0, origin)?),
            _ => {}
        }
    }
    Ok(XmlLoadout { includes, entries })
}

fn read_items<N: Node>(
    items: N,
    depth: usize,
    origin: &LoadoutOrigin,
) -> Result<Vec<EntrySpec>, String> {
    if depth >= MAX_LOADOUT_DEPTH {
        return Err(format!(
            "its items are nested deeper than {MAX_LOADOUT_DEPTH}"
        ));
    }
    let mut entries = Vec::new();
    for item in items.children() {
        if item.name() != "Item" {
            continue;
        }
        let class_name = item.attribute("itemName").unwrap_or_default();
        let mut nested = Vec::new();
        for child in item.children() {
            if child.name() == "Items" {
                nested.extend(read_items(child, depth.saturating_add(1), origin)?);
            }
        }
        entries.push(EntrySpec {
            origin: origin.clone(),
            port: item.attribute("portName").unwrap_or_default(),
            target: if class_name.is_empty() {
                TargetSpec::None
            } else {
                TargetSpec::ClassName(class_name)
            },
            loadout: (!nested.is_empty()).then(|| LoadoutSpec::Manual(Arc::from(nested))),
        });
    }
    Ok(entries)
}
