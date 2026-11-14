// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_dataforge::StructValue;

use crate::LoadoutPathOperation;
use crate::assortment;
use crate::loadout_path::PathRead;
use crate::manual_loadout::ManualLoadout;
use crate::read;
use crate::spec::{EntrySpec, LoadoutOrigin, LoadoutSpec, TargetSpec};
use crate::target_spec;

const XML_LOADOUT: &str = "SItemPortLoadoutXMLParams";
const OUTFIT_LOADOUT: &str = "SItemPortLoadoutOutfitParams";
const GENERATED_LOADOUT: &str = "SItemPortLoadoutRandomGeneratedParams";

/// Reads a manual, XML or outfit Loadout; `None` for any other struct.
pub(crate) fn read_loadout(
    loadout: &StructValue<'_>,
    record: &Arc<str>,
    paths: &PathRead<'_>,
) -> Option<LoadoutSpec> {
    if loadout.type_name() == XML_LOADOUT {
        let path = paths.field(loadout, "loadoutPath", LoadoutPathOperation::Copy);
        return (!path.is_empty()).then_some(LoadoutSpec::Xml(path));
    }
    if loadout.type_name() == OUTFIT_LOADOUT {
        return crate::outfit_read::read(loadout, record, false, paths).loadout;
    }
    if loadout.type_name() == GENERATED_LOADOUT {
        return assortment::read_assortments(loadout, paths);
    }
    loadout.field("entries")?;
    Some(LoadoutSpec::Manual(ManualLoadout::new(
        read::struct_items(loadout, "entries")
            .map(|entry| read_entry(entry, record, paths))
            .collect(),
    )))
}

fn read_entry(entry: &StructValue<'_>, record: &Arc<str>, paths: &PathRead<'_>) -> EntrySpec {
    let reference = entry
        .field("entityClassReference")
        .map_or(TargetSpec::None, |value| {
            target_spec::reference_target_at(entry, value, paths)
        });
    let target = match reference {
        TargetSpec::None => paths
            .class_name(entry, "entityClassName")
            .map_or(TargetSpec::None, TargetSpec::ClassName),
        other => other,
    };
    let port_proof = paths.field(entry, "itemPortName", LoadoutPathOperation::PortName);
    EntrySpec {
        origin: LoadoutOrigin::Record(Arc::clone(record)),
        port: port_proof.value().to_owned(),
        port_proof: port_proof.selection().is_some().then_some(port_proof),
        target,
        loadout: read::struct_field(entry, "loadout")
            .and_then(|loadout| read_loadout(loadout, record, paths)),
        xml_route: None,
        xml_use: None,
    }
}
