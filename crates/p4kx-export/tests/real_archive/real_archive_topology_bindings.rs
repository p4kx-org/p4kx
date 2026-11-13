// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, HashMap};

use p4kx_assembly::{Attachment, EntityNode, PortNode};
use p4kx_extraction::{RecordTree, StructValue, Value as Original};
use serde_json::{Value, json};

use super::super::{Declaration, TestResult};
use super::component_slots;
use crate::authored_definition_witness::{field, verify_state};
use crate::catalog_v3_consumer as consumer;
use crate::real_archive_auto_spawns::{original::Locations, provenance::Origins};
use crate::real_archive_item_components as components;
use crate::real_archive_item_durability as proof;

const LINK_BINDINGS: &str = "/placements/resourceNetworkLinkBindings/rows";
const ROOM_BINDINGS: &str = "/placements/roomAssociationBindings/rows";
const NODES: &str = "/placements/resourceNetworkNodes/rows";

#[derive(Default)]
pub(super) struct Counts {
    pub(super) hardpoints: usize,
    pub(super) rooms: usize,
}

pub(super) fn compare(
    catalog: &Value,
    origins: &Origins<'_>,
    tree: &RecordTree<'_>,
    assembly: &EntityNode,
    relays: &BTreeMap<usize, u32>,
) -> TestResult<Counts> {
    let loadouts = proof::rows(catalog, "/loadouts")?;
    let guid = tree.guid().to_string();
    let mut matches = loadouts
        .iter()
        .enumerate()
        .filter(|(_, row)| row["entityGuid"] == guid);
    let (root, loadout) = matches.next().ok_or("selected original ship loadout")?;
    assert!(
        matches.next().is_none(),
        "the ship has a single selected loadout"
    );
    let scope = Scope {
        catalog,
        origins,
        owner: tree.record_index(),
        locations: Locations::read(tree.root()),
        source_uses: proof::indexed(catalog, "/provenance/uses", "id")?,
        root,
        ports: assembly.ports(),
    };
    compare_relays(loadout, relays, scope.ports)?;
    let hardpoints = scope.declarations(tree, Declaration::Hardpoint)?;
    let rooms = scope.declarations(tree, Declaration::Room)?;
    Ok(Counts { hardpoints, rooms })
}

fn compare_relays(
    loadout: &Value,
    relays: &BTreeMap<usize, u32>,
    ports: &[PortNode],
) -> TestResult {
    let actual = proof::rows(loadout, "/ports")?;
    assert_eq!(
        actual.len(),
        ports.len(),
        "the original root Loadout port order is retained"
    );
    for (&ordinal, &target) in relays {
        let port = ports
            .get(ordinal)
            .ok_or("selected original relay port ordinal")?;
        let Attachment::Entity(entity) = port.attachment() else {
            return Err("source-selected relay attachment".into());
        };
        let row = actual
            .get(ordinal)
            .ok_or("selected relay port in scoped Loadout")?;
        assert!(
            row["name"].as_str() == Some(port.name())
                && row["itemGuid"].as_str() == Some(entity.guid().to_string().as_str())
                && row
                    .pointer("/sourceConstraints/attachedItem/targetRecordIndex/value")
                    .and_then(Value::as_u64)
                    == Some(u64::from(target)),
            "the contextual relay port retains the independent physical attachment target"
        );
    }
    Ok(())
}

struct Scope<'read, 'catalog, 'source> {
    catalog: &'catalog Value,
    origins: &'read Origins<'catalog>,
    owner: u32,
    locations: Locations<'source>,
    source_uses: HashMap<&'catalog str, &'catalog Value>,
    root: usize,
    ports: &'read [PortNode],
}

impl<'source> Scope<'_, '_, 'source> {
    fn declarations(
        &self,
        tree: &RecordTree<'source>,
        declaration: Declaration,
    ) -> TestResult<usize> {
        let (table, kind, source_field, exported) = declaration.fields();
        let (binding_table, ordinal_key) = match declaration {
            Declaration::Hardpoint => (LINK_BINDINGS, "linkOrdinal"),
            Declaration::Room => (ROOM_BINDINGS, "containerOrdinal"),
        };
        let mut rows = components::component_rows(self.catalog, table)?;
        let mut bindings = self.root_bindings(binding_table, ordinal_key)?;
        let mut count = 0;
        for (component, data) in component_slots(tree).filter(|(_, data)| data.type_name() == kind)
        {
            let row = rows
                .remove(&(self.owner, component))
                .ok_or("missing physical topology owner")?;
            assert!(
                proof::text(row, "/ownerGuid/value")? == tree.guid().to_string(),
                "the topology row retains the physical Record"
            );
            count += self.compare_array(
                data.field(source_field),
                field(row, exported)?,
                declaration,
                component,
                ordinal_key,
                &mut bindings,
            )?;
        }
        assert!(
            rows.keys().all(|(owner, _)| *owner != self.owner) && bindings.is_empty(),
            "every row and binding of the root corresponds to a physical occurrence"
        );
        Ok(count)
    }

    fn root_bindings(
        &self,
        table: &str,
        ordinal_key: &str,
    ) -> TestResult<BTreeMap<(usize, usize), &Value>> {
        let mut bindings = BTreeMap::new();
        for row in proof::rows(self.catalog, table)? {
            let context = field(row, "context")?;
            if context["loadoutOrdinal"] != self.root
                || !proof::rows(context, "/portOrdinals")?.is_empty()
                || table == LINK_BINDINGS && row["family"] != "hardpoint"
            {
                continue;
            }
            let owner = proof::number(row, "/ownerRecordIndex/value")?;
            let component = usize::try_from(proof::number(row, "/componentOrdinal/value")?)?;
            let index = ordinal(row, ordinal_key)?;
            assert!(
                owner == u64::from(self.owner)
                    && bindings.insert((component, index), row).is_none(),
                "each contextual binding has one original owner and array slot"
            );
        }
        Ok(bindings)
    }

    fn compare_array(
        &self,
        raw: Option<&Original<'source>>,
        cell: &Value,
        declaration: Declaration,
        component: usize,
        ordinal_key: &str,
        bindings: &mut BTreeMap<(usize, usize), &Value>,
    ) -> TestResult<usize> {
        let Some(raw @ Original::Array(originals)) = raw else {
            verify_state(raw, cell)?;
            return Ok(0);
        };
        self.origin(cell, raw, "$value")?;
        let mut count = 0;
        let published = proof::rows(cell, "/value")?;
        assert_eq!(
            originals.len(),
            published.len(),
            "all authored topology array slots"
        );
        for (slot, (source, row)) in originals.iter().zip(published).enumerate() {
            if let Original::Struct(source) = source {
                let row = consumer::present(row)?;
                assert_eq!(
                    ordinal(row, ordinal_key)?,
                    slot,
                    "original topology array slot"
                );
                let binding = bindings
                    .remove(&(component, slot))
                    .ok_or("original topology slot has no contextual binding")?;
                self.fields(source, row, binding, declaration)?;
                count += 1;
            } else {
                verify_state(Some(source), row)?;
            }
        }
        Ok(count)
    }

    fn fields(
        &self,
        data: &StructValue<'source>,
        row: &Value,
        binding: &Value,
        declaration: Declaration,
    ) -> TestResult {
        for name in declaration.members() {
            let actual = field(row, name)?;
            let Some(raw @ Original::String(expected)) = data.field(name) else {
                verify_state(data.field(name), actual)?;
                continue;
            };
            assert!(
                consumer::present(actual)?.as_str() == Some(expected),
                "original {name} is retained"
            );
            self.origin(actual, raw, "$value")?;
        }
        match declaration {
            Declaration::Hardpoint => {
                for name in ["port1", "port2"] {
                    self.endpoint(data.field(name), field(binding, name)?)?;
                }
            }
            Declaration::Room => {
                let selector = "resourceNetworkItemportName";
                self.endpoint(data.field(selector), field(binding, "port")?)?;
                let nav_mesh = "shouldRequestNavMeshRegenerationOnLoad";
                let cell = field(row, nav_mesh)?;
                if let Some(raw @ Original::Bool(expected)) = data.field(nav_mesh) {
                    assert!(
                        consumer::present(cell)?.as_bool() == Some(*expected),
                        "the original room NavMesh flag is retained"
                    );
                    self.origin(cell, raw, "$value")?;
                } else {
                    verify_state(data.field(nav_mesh), cell)?;
                }
            }
        }
        Ok(())
    }

    fn endpoint(&self, source: Option<&Original<'source>>, binding: &Value) -> TestResult {
        let result = consumer::present(binding)?;
        let state = proof::text(result, "/state")?;
        if let Some(raw) = source {
            self.selector(raw, binding)?;
        }
        let Some(Original::String(name)) = source else {
            assert!(
                matches!(state, "unspecified" | "unavailable"),
                "a missing selector has explicit state"
            );
            return Ok(());
        };
        if name.is_empty() {
            assert_eq!(
                state, "unspecified",
                "an empty authored selector is unspecified"
            );
            return Ok(());
        }
        let candidates = match state {
            "resolved" | "unplaced" => std::slice::from_ref(field(result, "candidate")?),
            "ambiguous" | "unavailable" => proof::rows(result, "/candidates")?,
            "missing" => &[],
            _ => return Err("unknown named endpoint state".into()),
        };
        let mut actual = Vec::new();
        for candidate in candidates {
            if let Some(ordinal) = field(candidate, "loadoutPortOrdinal")?.as_u64() {
                let ordinal = usize::try_from(ordinal)?;
                let port = self
                    .ports
                    .get(ordinal)
                    .ok_or("endpoint candidate port ordinal")?;
                assert!(
                    port.name().eq_ignore_ascii_case(name),
                    "the endpoint candidate names an original port in its own context"
                );
                self.node(ordinal, candidate)?;
                actual.push(ordinal);
            }
        }
        let expected: Vec<_> = self
            .ports
            .iter()
            .enumerate()
            .filter_map(|(ordinal, port)| port.name().eq_ignore_ascii_case(name).then_some(ordinal))
            .collect();
        assert!(
            actual == expected,
            "endpoint candidates cover all matching direct original Loadout ports"
        );
        Ok(())
    }

    fn selector(&self, raw: &Original<'source>, binding: &Value) -> TestResult {
        let ordinal = self.locations.ordinal(raw)?;
        let location = json!({
            "type": "subresource",
            "namespace": "dataforgeValue",
            "indices": [self.owner.to_string(), ordinal.to_string()],
            "field": "$value",
        });
        let mut matching = false;
        let mut encrypted = false;
        for reference in proof::rows(binding, "/sourceUsesRefs")? {
            let id = reference.as_str().ok_or("endpoint source-use ID")?;
            let used = self
                .source_uses
                .get(id)
                .ok_or("endpoint source-use closure")?;
            matching |= used["role"] == "selector" && used["location"] == location;
            encrypted |= used["effectiveEncryption"] == "encrypted";
        }
        assert!(
            matching,
            "the binding retains the physical authored selector"
        );
        if encrypted {
            assert!(
                binding["containsEncryptedSource"] == true,
                "the binding retains its encrypted contributor"
            );
        }
        Ok(())
    }

    fn node(&self, ordinal: usize, candidate: &Value) -> TestResult {
        let matches = proof::rows(self.catalog, NODES)?.iter().filter(|row| {
            row.pointer("/position/loadoutOrdinal") == Some(&Value::from(self.root))
                && row
                    .pointer("/position/portOrdinals")
                    .is_some_and(|route| route == &json!([ordinal]))
        });
        let mut matches = matches;
        let node = matches.next().ok_or("contextual endpoint candidate node")?;
        assert!(
            matches.next().is_none(),
            "a contextual node is emitted once"
        );
        assert!(
            field(node, "originalPort")? == field(candidate, "physicalPort")?,
            "the binding and placement node retain the same physical port"
        );
        Ok(())
    }

    fn origin(&self, cell: &Value, raw: &Original<'source>, part: &str) -> TestResult {
        self.origins
            .occurrence(cell, self.owner, self.locations.ordinal(raw)?, part)
    }
}

fn ordinal(row: &Value, name: &str) -> TestResult<usize> {
    let value = consumer::present(field(row, name)?)?
        .as_u64()
        .ok_or("original topology array ordinal")?;
    Ok(usize::try_from(value)?)
}
