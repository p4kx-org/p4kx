// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{ArchiveFiles, Attachment, EntityNode, LoadoutResolver, PortNode};
use p4kx_extraction::{Archive, DataForge, RecordTree, TreeBuilder};
use serde_json::Value;

use super::{CONTROLLER_TYPE, IFCS_TYPE, Result, Selection, THRUSTER_TYPE};
use crate::real_archive_item_durability::{number, rows, text};

const INSTALLED: &str = "/placements/installedConfigurations/rows";

pub(super) fn check_vehicle<'source>(
    archive: &'source Archive,
    forge: &'source DataForge<'source>,
    catalog: &Value,
    selection: &Selection<'source>,
    category: &str,
    controller: Option<&RecordTree<'source>>,
) -> Result {
    let guid = selection.tree.guid().to_string();
    assert!(
        rows(catalog, "/ships")?.iter().any(|ship| {
            ship.get("guid").and_then(Value::as_str) == Some(guid.as_str())
                && ship.get("vehicleKind").and_then(Value::as_str) == Some(category)
        }),
        "selected source is the expected vehicle category"
    );
    let loadout = rows(catalog, "/loadouts")?
        .iter()
        .find(|row| row.get("entityGuid").and_then(Value::as_str) == Some(guid.as_str()))
        .ok_or("matching selected vehicle Loadout")?;
    let record = forge
        .record(selection.tree.record_index())
        .ok_or("vehicle source Record")?;
    let files = ArchiveFiles::new(archive);
    let mut resolver = LoadoutResolver::new(forge)
        .with_files(&files)
        .stopping_at_docked_vehicles();
    let original = resolver
        .resolve(&record)
        .map_err(|_| "read original resolved Loadout")?;
    let census = Inventory::read(forge, original.ports(), loadout, controller)?;
    census.verify(controller.is_some());
    assert!(
        !rows(catalog, INSTALLED)?.iter().any(|row| {
            row.pointer("/shipRecordIndex/value")
                .and_then(Value::as_u64)
                == Some(u64::from(selection.tree.record_index()))
        }),
        "the exact root filter includes no separate configuration owner rows"
    );
    Ok(())
}

#[derive(Default)]
struct Inventory {
    controllers: usize,
    thrusters: usize,
    expected_controller: usize,
}

impl Inventory {
    fn read<'source>(
        forge: &'source DataForge<'source>,
        ports: &[PortNode],
        loadout: &Value,
        controller: Option<&RecordTree<'source>>,
    ) -> Result<Self> {
        let probes = [CONTROLLER_TYPE, IFCS_TYPE, THRUSTER_TYPE]
            .map(|kind| forge.pointer_probe("Components", kind));
        let [Some(flight), Some(ifcs), Some(thruster)] = probes else {
            return Err("original flight family declarations".into());
        };
        let mut census = Self::default();
        let mut trees = TreeBuilder::new();
        let root = rows(loadout, "/ports")?;
        let mut pending = vec![(ports, root)];
        while let Some((ports, output_ports)) = pending.pop() {
            assert_eq!(ports.len(), output_ports.len(), "resolved port cardinality");
            for (port, output) in ports.iter().zip(output_ports) {
                assert!(
                    text(output, "/name")? == port.name(),
                    "authored ordered port"
                );
                let Attachment::Entity(child) = port.attachment() else {
                    if matches!(port.attachment(), Attachment::Empty) {
                        pending.push((port.nested(), rows(output, "/ports")?));
                    }
                    continue;
                };
                if child.is_docked_vehicle() {
                    continue;
                }
                pending.push((child.ports(), rows(output, "/ports")?));
                let Some(index) = child.record_index() else {
                    continue;
                };
                let record = forge.record(index).ok_or("attached Record identity")?;
                let control = trees.holds(&flight, &record)? || trees.holds(&ifcs, &record)?;
                let thrust = trees.holds(&thruster, &record)?;
                census.controllers += usize::from(control);
                census.thrusters += usize::from(thrust);
                if controller.is_some_and(|owned| owned.record_index() == index) {
                    assert!(
                        control,
                        "known installed controller is a physical configuration"
                    );
                    check_port(output, child, index)?;
                    census.expected_controller += 1;
                }
            }
        }
        Ok(census)
    }

    fn verify(&self, expected_controller: bool) {
        if expected_controller {
            assert!(
                self.expected_controller > 0,
                "the selected controller is actually attached"
            );
        } else {
            assert_eq!(
                self.controllers, 0,
                "selected ground Loadout has no flight controller"
            );
            assert_eq!(
                self.thrusters, 0,
                "selected ground Loadout has no flight thruster"
            );
        }
    }
}

fn check_port(output: &Value, child: &EntityNode, index: u32) -> Result {
    let attached = output
        .pointer("/sourceConstraints/attachedItem")
        .ok_or("exact resolved port attachment")?;
    let target = child
        .attachment_target()
        .ok_or("authored physical attachment selector")?;
    assert!(
        target.guid() == child.guid(),
        "actual target matches attached original GUID"
    );
    let guid = child.guid().to_string();
    assert!(
        text(output, "/itemGuid")? == guid,
        "original port target GUID"
    );
    assert!(
        text(attached, "/targetGuid/value")? == guid,
        "sourced physical target GUID"
    );
    assert_eq!(
        number(attached, "/targetRecordIndex/value")?,
        u64::from(index),
        "sourced physical target Record position"
    );
    assert!(
        rows(attached, "/targetGuid/sourceUsesRefs")?
            .iter()
            .any(Value::is_string),
        "selected attachment retains direct source evidence"
    );
    Ok(())
}
