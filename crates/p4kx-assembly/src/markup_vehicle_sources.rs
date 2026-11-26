// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use crate::loadout_xml::{MissingAttribute, Node, Scope};
use crate::vehicle::{ITEM_SUBTYPES_ATTRIBUTE, ITEM_TYPE_ATTRIBUTE, PORT_OBSERVED_ATTRIBUTES};
use crate::{ValueOrigin, VehicleAttributeEvidence};

use super::AttributeOrigins;

pub(super) fn original_sites(
    node: Node<'_, '_>,
    scope: &Scope<'_, '_>,
    attributes: &[(String, String)],
) -> AttributeOrigins {
    let mut origins = AttributeOrigins::default();
    let totals = scope.observes_vehicle_totals();
    let selection = totals || scope.observes_vehicle_port_attributes();
    if selection {
        origins.set_element(scope.vehicle_element_site(node));
        retain(
            &mut origins,
            node,
            scope,
            attributes,
            "name",
            MissingAttribute::Element,
        );
    }
    retain(
        &mut origins,
        node,
        scope,
        attributes,
        "id",
        absence_scope(selection),
    );
    let port = node.name().eq_ignore_ascii_case("ItemPort");
    retain_port_attributes(&mut origins, node, scope, attributes, port);
    retain(
        &mut origins,
        node,
        scope,
        attributes,
        "flags",
        absence_scope(port),
    );
    let is_part = node.name().eq_ignore_ascii_case("Part");
    if selection {
        retain(
            &mut origins,
            node,
            scope,
            attributes,
            "skipPart",
            absence_scope(is_part),
        );
    }
    if totals {
        let part_absence = absence_scope(is_part);
        for (name, missing) in [
            ("class", MissingAttribute::Omit),
            ("mass", part_absence),
            ("damageMax", part_absence),
        ] {
            retain(&mut origins, node, scope, attributes, name, missing);
        }
    }
    origins
}

fn retain_port_attributes(
    origins: &mut AttributeOrigins,
    node: Node<'_, '_>,
    scope: &Scope<'_, '_>,
    attributes: &[(String, String)],
    port: bool,
) {
    if !scope.observes_vehicle_port_attributes() {
        return;
    }
    let patch_donor = scope.is_vehicle_patch()
        && attributes
            .iter()
            .any(|(name, value)| name.eq_ignore_ascii_case("id") && !value.is_empty());
    let item_type = node.name().eq_ignore_ascii_case("Type");
    if item_type || patch_donor {
        for name in [ITEM_TYPE_ATTRIBUTE, ITEM_SUBTYPES_ATTRIBUTE] {
            retain(
                origins,
                node,
                scope,
                attributes,
                name,
                absence_scope(item_type),
            );
        }
    }
    if port || patch_donor {
        for name in PORT_OBSERVED_ATTRIBUTES {
            retain(origins, node, scope, attributes, name, absence_scope(port));
        }
    }
}

const fn absence_scope(observe: bool) -> MissingAttribute {
    if observe {
        MissingAttribute::Element
    } else {
        MissingAttribute::Omit
    }
}

fn retain(
    origins: &mut AttributeOrigins,
    node: Node<'_, '_>,
    scope: &Scope<'_, '_>,
    attributes: &[(String, String)],
    name: &str,
    missing: MissingAttribute,
) {
    let Some(site) = scope.vehicle_site(node, name, missing) else {
        return;
    };
    origins.assign(
        name,
        ValueOrigin::Base,
        Some(Arc::new(VehicleAttributeEvidence::new(site, Vec::new()))),
    );
    for (ordinal, (raw_name, _)) in attributes
        .iter()
        .enumerate()
        .filter(|(_, (raw_name, _))| raw_name.eq_ignore_ascii_case(name))
        .skip(1)
    {
        let proof = scope
            .vehicle_site_at(node, ordinal, raw_name)
            .map(|site| Arc::new(VehicleAttributeEvidence::new(site, Vec::new())));
        origins.retain_donor(raw_name, ordinal, proof);
    }
}
