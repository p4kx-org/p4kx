// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use crate::loadout_file_trace::FileTrace;
use crate::loadout_xml::{Node, Scope};
use crate::manual_loadout::ManualLoadout;
use crate::resolver_observation::Observation;
use crate::spec::{EntrySpec, LoadoutOrigin, LoadoutSpec, TargetSpec};
use crate::{
    LoadoutDocumentKind, LoadoutError, LoadoutFailurePath, LoadoutFiles, LoadoutPath,
    LoadoutRequest, MAX_LOADOUT_DEPTH,
};

#[path = "xml_include.rs"]
mod include;

pub(crate) use include::load_observed;

struct XmlLoadout {
    includes: Vec<LoadoutPath>,
    entries: Vec<EntrySpec>,
}

pub(crate) struct XmlEntries {
    pub(crate) entries: Arc<[EntrySpec]>,
    pub(crate) issues: Vec<LoadoutError>,
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
