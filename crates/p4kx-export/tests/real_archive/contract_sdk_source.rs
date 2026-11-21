// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use p4kx_export::catalog_queries::MissionPropertyLevel;
use p4kx_export::{Kind, is_kind_leftover};
use p4kx_extraction::{Guid, RecordTree, StructValue, Value};

use super::TestResult;

pub(super) type Key = (Guid, Option<Guid>);

pub(super) struct Site<'tree, 'source> {
    pub(super) handler: &'tree Value<'source>,
    pub(super) contract: &'tree Value<'source>,
    pub(super) sub: Option<&'tree Value<'source>>,
    pub(super) key: Key,
    pub(super) sets: [Option<usize>; 3],
}

pub(super) struct Set<'tree, 'source> {
    pub(super) location: String,
    pub(super) level: MissionPropertyLevel,
    pub(super) properties: &'tree [Value<'source>],
}

pub(super) struct Rows<'tree, 'source> {
    pub(super) sites: Vec<Site<'tree, 'source>>,
    pub(super) sets: Vec<Set<'tree, 'source>>,
}

impl<'tree, 'source> Rows<'tree, 'source> {
    pub(super) fn read(tree: &'tree RecordTree<'source>) -> TestResult<Self> {
        let mut output = Self {
            sites: Vec::new(),
            sets: Vec::new(),
        };
        for (index, value) in array(tree.root(), "generators")?.iter().enumerate() {
            let Some(handler) = structure(Some(value))? else {
                continue;
            };
            let path = format!("generators[{index}]");
            let set = output.register(
                parameter(handler, "contractParams")?,
                &format!("{path}.contractParams"),
                MissionPropertyLevel::Handler,
            )?;
            output.contracts(tree, value, handler, &path, set)?;
        }
        Ok(output)
    }

    fn contracts(
        &mut self,
        tree: &RecordTree<'source>,
        handler_value: &'tree Value<'source>,
        handler: &'tree StructValue<'source>,
        path: &str,
        handler_set: Option<usize>,
    ) -> TestResult {
        for container in [
            "contracts",
            "introContracts",
            "legacyContracts",
            "PVPBountyContract",
        ] {
            for (index, value) in array(handler, container)?.iter().enumerate() {
                let Some(contract) = structure(Some(value))? else {
                    continue;
                };
                let path = format!("{path}.{container}[{index}]");
                let set = self.register(
                    parameter(contract, "paramOverrides")?,
                    &format!("{path}.paramOverrides"),
                    MissionPropertyLevel::Contract,
                )?;
                let Some(guid) = admitted(tree, contract) else {
                    continue;
                };
                self.sites.push(Site {
                    handler: handler_value,
                    contract: value,
                    sub: None,
                    key: (guid, None),
                    sets: [handler_set, set, None],
                });
                self.subs(tree, handler_value, value, &path, [handler_set, set], guid)?;
            }
        }
        Ok(())
    }

    fn subs(
        &mut self,
        tree: &RecordTree<'source>,
        handler: &'tree Value<'source>,
        contract_value: &'tree Value<'source>,
        path: &str,
        sets: [Option<usize>; 2],
        guid: Guid,
    ) -> TestResult {
        let contract = structure(Some(contract_value))?.ok_or("original parent Struct")?;
        for (index, value) in array(contract, "subContracts")?.iter().enumerate() {
            let Some(sub) = structure(Some(value))? else {
                continue;
            };
            let Some(sub_guid) = admitted(tree, sub) else {
                continue;
            };
            let path = format!("{path}.subContracts[{index}]");
            let (parameters, path) = if let Some(parameters) = parameter(sub, "paramOverrides")? {
                (parameters, format!("{path}.paramOverrides"))
            } else {
                (sub, path)
            };
            let set = self.register(Some(parameters), &path, MissionPropertyLevel::SubContract)?;
            self.sites.push(Site {
                handler,
                contract: contract_value,
                sub: Some(value),
                key: (guid, Some(sub_guid)),
                sets: [sets[0], sets[1], set],
            });
        }
        Ok(())
    }

    fn register(
        &mut self,
        parameters: Option<&'tree StructValue<'source>>,
        path: &str,
        level: MissionPropertyLevel,
    ) -> TestResult<Option<usize>> {
        let Some(parameters) = parameters else {
            return Ok(None);
        };
        let properties = match parameters.field("propertyOverrides") {
            None | Some(Value::Null) => return Ok(None),
            Some(Value::Array(values)) => values.as_ref(),
            _ => return Err("authored property overrides must be an Array".into()),
        };
        let index = self.sets.len();
        self.sets.push(Set {
            location: format!("{path}.propertyOverrides"),
            level,
            properties,
        });
        Ok(Some(index))
    }

    pub(super) fn has_required_source_features(&self) -> bool {
        self.probe_keys().is_ok()
            && self.sites.iter().any(|site| site.sets.iter().any(Option::is_some))
            && self.sites.iter().any(|site| {
                let Value::Struct(contract) = site.contract else { return false; };
                ["template", "missionBrokerEntry"].into_iter().any(|name| {
                    matches!(contract.field(name), Some(Value::Reference(reference)) if reference.location().is_none())
                })
            })
    }

    pub(super) fn keys(&self) -> BTreeSet<Key> {
        self.sites.iter().map(|site| site.key).collect()
    }

    pub(super) fn probe_keys(&self) -> TestResult<[Key; 2]> {
        let parent = self
            .sites
            .iter()
            .find(|site| site.key.1.is_none())
            .ok_or("the explicit source needs an admitted parent")?;
        let sub = self
            .sites
            .iter()
            .find(|site| site.key.1.is_some())
            .ok_or("the explicit source needs an admitted sub-contract")?;
        Ok([parent.key, sub.key])
    }
}

fn admitted(tree: &RecordTree<'_>, source: &StructValue<'_>) -> Option<Guid> {
    if is_kind_leftover(
        Kind::Contracts,
        tree.record_path(),
        text(source, "debugName").unwrap_or_default(),
    ) {
        return None;
    }
    match source.field("id") {
        Some(Value::Guid(guid)) if *guid != Guid::default() => Some(*guid),
        _ => None,
    }
}

pub(super) fn array<'tree, 'source>(
    source: &'tree StructValue<'source>,
    name: &str,
) -> TestResult<&'tree [Value<'source>]> {
    match source.field(name) {
        None | Some(Value::Null) => Ok(&[]),
        Some(Value::Array(values)) => Ok(values),
        _ => Err("original declaration must be a nullable Array".into()),
    }
}

pub(super) fn structure<'tree, 'source>(
    value: Option<&'tree Value<'source>>,
) -> TestResult<Option<&'tree StructValue<'source>>> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Struct(value)) => Ok(Some(value)),
        _ => Err("original occurrence must be a nullable Struct".into()),
    }
}

fn parameter<'tree, 'source>(
    source: &'tree StructValue<'source>,
    name: &str,
) -> TestResult<Option<&'tree StructValue<'source>>> {
    structure(source.field(name))
}

pub(super) fn text<'source>(source: &StructValue<'source>, name: &str) -> Option<&'source str> {
    match source.field(name) {
        Some(Value::String(value)) => Some(*value),
        _ => None,
    }
}

pub(super) fn boolean(source: &StructValue<'_>, name: &str) -> Option<bool> {
    match source.field(name) {
        Some(Value::Bool(value)) => Some(*value),
        _ => None,
    }
}
