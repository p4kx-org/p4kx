// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;

use crate::LoadoutError;
use crate::manual_loadout::{self, ManualLoadout};
use crate::resolver::{Entries, EntriesResult, LoadoutResolver};
use crate::spec::{EntrySpec, LoadoutSpec, TargetSpec};

/// Returns the key a port name is matched by: the name in ASCII lower case.
#[must_use]
pub fn port_key(name: &str) -> String {
    name.to_ascii_lowercase()
}

/// Lays the entries `over` on top of `entries`: an entry of a port `entries` already has replaces
/// it, except that an entry that names no Entity and has a manual Loadout of its own keeps the
/// earlier entry's Entity and has its nested entries laid over the earlier ones, recursively.
pub(crate) fn overlay(entries: &mut Vec<EntrySpec>, over: impl IntoIterator<Item = EntrySpec>) {
    for entry in over {
        let key = port_key(&entry.port);
        let at = entries.iter().position(|old| port_key(&old.port) == key);
        let Some(old) = at.and_then(|at| entries.get_mut(at)) else {
            entries.push(entry);
            continue;
        };
        let adds = entry.target == TargetSpec::None;
        if !adds {
            *old = entry;
            continue;
        }
        let Some(extra) = entry.loadout else { continue };
        old.loadout = Some(match (old.loadout.take(), extra) {
            (Some(LoadoutSpec::Manual(base)), LoadoutSpec::Manual(extra)) => {
                let mut nested = manual_loadout::entries(&base, base.usage())
                    .map(Cow::into_owned)
                    .collect();
                overlay(
                    &mut nested,
                    manual_loadout::entries(&extra, extra.usage()).map(Cow::into_owned),
                );
                LoadoutSpec::Manual(ManualLoadout::new(nested.into()))
            }
            (_, extra) => extra,
        });
    }
}

/// Replaces the first item of each port `overrides` names, ignoring ASCII case, and appends the
/// rest in order; `port` returns the port name of an item.
pub(crate) fn merge_by_port<T>(
    items: &mut Vec<T>,
    overrides: impl IntoIterator<Item = T>,
    port: impl Fn(&T) -> &str,
) {
    let mut index: HashMap<String, usize> = HashMap::with_capacity(items.len());
    for (at, item) in items.iter().enumerate() {
        index.entry(port_key(port(item))).or_insert(at);
    }
    for item in overrides {
        let key = port_key(port(&item));
        if let Some(old) = index.get(&key).and_then(|at| items.get_mut(*at)) {
            *old = item;
        } else {
            index.insert(key, items.len());
            items.push(item);
        }
    }
}

impl LoadoutResolver<'_> {
    /// Returns the entries of the layers of an outfit laid over each other with [`overlay`]; the
    /// issues of every layer are kept, and a layer that cannot be read is an issue unless no layer
    /// gives any entry.
    pub(crate) fn layered_entries(&mut self, layers: &[LoadoutSpec]) -> EntriesResult {
        let mut list: Vec<EntrySpec> = Vec::new();
        let mut issues: Vec<LoadoutError> = Vec::new();
        let mut failure = None;
        for layer in layers {
            if self.observation_stopped() {
                return Err(LoadoutError::ObservationStopped);
            }
            match self.entries(Some(layer)) {
                Ok(entries) => {
                    issues.extend(entries.issues.iter().cloned());
                    overlay(&mut list, entries.specs().map(Cow::into_owned));
                }
                Err(error) => {
                    failure = Some(error.clone());
                    issues.push(error);
                }
            }
        }
        match failure {
            Some(error) if list.is_empty() => Err(error),
            _ => Ok(Entries {
                list: Arc::from(list),
                issues: Arc::from(issues),
                xml_use: None,
            }),
        }
    }
}
