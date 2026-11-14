// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::loadout_file_trace::FileTrace;
use crate::loadout_xml::{Node, Scope};
use crate::loadout_xml_use::{XmlInclude, XmlRoute};
use crate::manual_loadout::ManualLoadout;
use crate::merge::merge_by_port;
use crate::resolver_observation::{self, LoadoutReadEvent, Observation};
use crate::spec::{EntrySpec, LoadoutOrigin, LoadoutSpec, TargetSpec};
use crate::{
    LoadoutDocumentKind, LoadoutError, LoadoutFailurePath, LoadoutFiles, LoadoutPath,
    LoadoutRequest, MAX_LOADOUT_DEPTH,
};

/// The most XML Loadouts one XML Loadout includes through nested `<Loadouts>`.
pub const MAX_XML_INCLUDE_DEPTH: usize = 8;

/// The most XML files one resolution reads, including its root and nested includes.
pub const MAX_XML_INCLUDES: usize = 256;

struct XmlLoadout {
    includes: Vec<LoadoutPath>,
    entries: Vec<EntrySpec>,
}

pub(crate) struct XmlEntries {
    pub(crate) entries: Vec<EntrySpec>,
    pub(crate) issues: Vec<LoadoutError>,
}

pub(crate) fn load_observed(
    files: &dyn LoadoutFiles,
    path: &LoadoutPath,
    caller: &LoadoutRequest,
    observation: Option<&Observation<'_>>,
    trace: &mut FileTrace,
) -> Result<XmlEntries, LoadoutError> {
    let mut loader = Loader {
        observation,
        files,
        trace,
        stack: Vec::new(),
        include_paths: Vec::new(),
        done: HashMap::new(),
        loads: 0,
        issues: Vec::new(),
    };
    let entries = loader.load(path, caller);
    let operation = loader
        .done
        .get(&path.to_ascii_lowercase())
        .and_then(|done| done.operation);
    loader.trace.set_xml(operation);
    entries.map(|entries| XmlEntries {
        entries,
        issues: loader.issues,
    })
}

struct CachedLoadout {
    result: Result<Vec<EntrySpec>, LoadoutError>,
    operation: Option<usize>,
}

struct Loader<'l> {
    observation: Option<&'l Observation<'l>>,
    files: &'l dyn LoadoutFiles,
    trace: &'l mut FileTrace,
    stack: Vec<String>,
    include_paths: Vec<LoadoutPath>,
    done: HashMap<String, CachedLoadout>,
    loads: usize,
    issues: Vec<LoadoutError>,
}

impl Loader<'_> {
    fn load(
        &mut self,
        path: &LoadoutPath,
        caller: &LoadoutRequest,
    ) -> Result<Vec<EntrySpec>, LoadoutError> {
        if self.observation.is_some_and(Observation::stopped) {
            return Err(LoadoutError::ObservationStopped);
        }
        if let Some(done) = self.done.get(&path.to_ascii_lowercase()) {
            return done.result.clone();
        }
        let failed = |reason: String| LoadoutError::XmlLoadout {
            path: path.to_string(),
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
        let first = self.trace.position();
        let loadout = match load_file(self.files, path, caller, self.observation, self.trace) {
            Ok(loadout) => loadout,
            Err(error) => {
                self.done.insert(
                    path.to_ascii_lowercase(),
                    CachedLoadout {
                        result: Err(error.clone()),
                        operation: self.trace.captured(first),
                    },
                );
                return Err(error);
            }
        };
        self.stack.push(path.to_string());
        let mut entries = self.load_includes(&loadout.includes, self.trace.captured(first))?;
        self.stack.pop();
        merge_into(&mut entries, loadout.entries);
        self.done.insert(
            path.to_ascii_lowercase(),
            CachedLoadout {
                result: Ok(entries.clone()),
                operation: self.trace.captured(first),
            },
        );
        Ok(entries)
    }

    fn load_include(
        &mut self,
        include: &LoadoutPath,
        caller: &LoadoutRequest,
    ) -> Result<Vec<EntrySpec>, LoadoutError> {
        if self.observation.is_none() {
            return self.load(include, caller);
        }
        self.include_paths.push(include.clone());
        let mut result = self.load(include, caller);
        if let Err(LoadoutError::MissingXmlLoadout { path }) = &mut result {
            path.selected_by(&self.include_paths);
        }
        self.include_paths.pop();
        result
    }

    fn load_includes(
        &mut self,
        paths: &[LoadoutPath],
        parent: Option<usize>,
    ) -> Result<Vec<EntrySpec>, LoadoutError> {
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
            let caller = LoadoutRequest::Xml {
                path: include.clone(),
            };
            if self.loads >= MAX_XML_INCLUDES && !self.done.contains_key(&key) {
                resolver_observation::notify(
                    self.observation,
                    LoadoutReadEvent::InventoryIncomplete,
                )?;
                let error = LoadoutError::XmlLoadout {
                    path: include.to_string(),
                    reason: format!(
                        "more than {MAX_XML_INCLUDES} Loadouts are read; the rest are left out"
                    ),
                };
                self.trace
                    .refused(parent, &caller, &error, self.observation)?;
                self.issues.push(error);
                break;
            }

            let result = self.load_include(include, &caller);
            let edge = if let Some(done) = self.done.get(&key) {
                self.trace
                    .dependency(parent, done.operation, &caller, self.observation)?
            } else {
                if let Err(error) = &result
                    && !matches!(error, LoadoutError::ObservationStopped)
                {
                    self.trace
                        .refused(parent, &caller, error, self.observation)?;
                }
                None
            };
            match result {
                Ok(included) => merge_into(&mut entries, through_include(included, edge)),
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
    path: &LoadoutPath,
    caller: &LoadoutRequest,
    observation: Option<&Observation<'_>>,
    trace: &mut FileTrace,
) -> Result<XmlLoadout, LoadoutError> {
    let failed = |reason| LoadoutError::XmlLoadout {
        path: path.to_string(),
        reason,
    };
    let first = trace.position();
    let bytes = crate::loadout_file_events::read(
        files,
        path,
        caller,
        LoadoutDocumentKind::XmlLoadout,
        observation,
        trace,
    )
    .map_err(|error| match error {
        crate::LoadoutFileError::ObservationStopped => LoadoutError::ObservationStopped,
        _ if observation.is_some_and(Observation::stopped) => LoadoutError::ObservationStopped,
        crate::LoadoutFileError::NotFound => LoadoutError::MissingXmlLoadout {
            path: LoadoutFailurePath::from_attempts(path.clone(), trace.attempts(first, 1)),
        },
        error => failed(error.to_string()),
    })?;
    let result = crate::loadout_xml::parse(&bytes, observation, |root, scope| {
        read_root(root, &LoadoutOrigin::File(Arc::clone(path.text())), scope)
    });
    crate::loadout_file_events::parsed(
        bytes,
        result.as_ref().map(|_| ()).map_err(String::as_str),
        observation,
    )?;
    result.map_err(failed)
}

fn through_include(mut entries: Vec<EntrySpec>, edge: Option<XmlInclude>) -> Vec<EntrySpec> {
    let Some(edge) = edge.filter(|_| !entries.is_empty()) else {
        return entries;
    };
    let edge = Arc::new(edge);
    let mut routes = HashMap::new();
    for entry in &mut entries {
        entry.xml_route = Some(XmlRoute::prepend(
            entry.xml_route.take(),
            &edge,
            &mut routes,
        ));
    }
    entries
}

fn merge_into(entries: &mut Vec<EntrySpec>, overrides: Vec<EntrySpec>) {
    merge_by_port(entries, overrides, |entry| &entry.port);
}

fn read_root(
    root: Node<'_, '_>,
    origin: &LoadoutOrigin,
    scope: &Scope<'_, '_>,
) -> Result<XmlLoadout, String> {
    if root.name() != "Loadout" {
        return Err(format!("its root element is {}, not Loadout", root.name()));
    }
    let mut includes = Vec::new();
    let mut entries = Vec::new();
    for child in root.children() {
        match child.name() {
            "Loadouts" => {
                for include in child.children() {
                    if include
                        .attribute("loadout")
                        .is_some_and(|value| !value.is_empty())
                        && let Some(path) = scope.path(include, "loadout")?
                    {
                        includes.push(path);
                    }
                }
            }
            "Items" => entries.extend(read_items(child, 0, origin, scope)?),
            _ => {}
        }
    }
    Ok(XmlLoadout { includes, entries })
}

fn read_items(
    items: Node<'_, '_>,
    depth: usize,
    origin: &LoadoutOrigin,
    scope: &Scope<'_, '_>,
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
        let class_name = scope.class_name(item, "itemName")?;
        let mut nested = Vec::new();
        for child in item.children() {
            if child.name() == "Items" {
                nested.extend(read_items(child, depth.saturating_add(1), origin, scope)?);
            }
        }
        entries.push(EntrySpec {
            origin: origin.clone(),
            port: item.attribute("portName").unwrap_or_default().to_owned(),
            port_proof: scope.port_name(item, "portName")?,
            target: class_name.map_or(TargetSpec::None, TargetSpec::ClassName),
            loadout: (!nested.is_empty())
                .then(|| LoadoutSpec::Manual(ManualLoadout::new(Arc::from(nested)))),
            xml_route: None,
            xml_use: None,
        });
    }
    Ok(entries)
}
