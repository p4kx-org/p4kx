// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use crate::loadout_xml::{MissingAttribute, Node, Scope};
use crate::vehicle::PORT_OBSERVED_ATTRIBUTES;
use crate::{ValueOrigin, VehicleAttributeEvidence};

use super::AttributeOrigins;

pub(super) fn original_sites(
    node: Node<'_, '_>,
    scope: &Scope<'_, '_>,
    attributes: &[(String, String)],
) -> AttributeOrigins {
    let mut origins = AttributeOrigins::default();
    if scope.observes_vehicle_totals() {
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
        if scope.observes_vehicle_totals() {
            MissingAttribute::Element
        } else {
            MissingAttribute::Omit
        },
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
    if scope.observes_vehicle_totals() {
        let is_part = node.name().eq_ignore_ascii_case("Part");
        for name in ["mass", "damageMax", "skipPart"] {
            retain(
                &mut origins,
                node,
                scope,
                attributes,
                name,
                absence_scope(is_part),
            );
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
