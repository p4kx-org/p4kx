// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourceLocation;

use crate::paint_provenance::PaintSelectionInput;
use crate::spec::EntitySpec;
use crate::target_spec::TargetSpec;
use crate::{
    EntityNode, PaintPortMatch, PaintTagAncestry, PortDefinition, PortNode, PortSource,
    VehicleImplementation, VehicleItemPort,
};

use super::PaintItem;

pub(super) fn selected(
    spec: &EntitySpec,
    root: &EntityNode,
    port: &PortNode,
    item: &EntityNode,
    paint: &PaintItem,
    offers: Option<&PaintPortMatch<'_>>,
) -> Result<Vec<PaintSelectionInput>, &'static str> {
    let mut sources = Vec::new();
    sources.extend(
        item.classification()
            .item_type_location()
            .map(PaintSelectionInput::DataForge),
    );
    match port.source() {
        PortSource::PortDefault => sources.extend(default_item(spec, port, item)),
        PortSource::Loadout => {
            if let Some(target) = item
                .attachment_target()
                .filter(|target| target.guid() == item.guid())
                && let Some(selection) = target.class_selection()
                && let Some(usage) = target.class_xml_use()
                && let Some(attributes) = usage.paint_attributes(selection)
            {
                sources.extend(
                    attributes
                        .into_iter()
                        .map(|(entry, location)| PaintSelectionInput::Xml(entry, location)),
                );
            }
        }
        PortSource::Replaced => {
            sources.extend(replacement_guid(item));
            if let Some(offers) = offers {
                replacement_tags(&mut sources, root, port, item, paint, offers);
                for ancestry in offers.ancestries() {
                    append_ancestry(&mut sources, ancestry)?;
                }
            }
        }
        _ => {}
    }
    Ok(sources)
}

fn default_item(
    spec: &EntitySpec,
    port: &PortNode,
    item: &EntityNode,
) -> Option<PaintSelectionInput> {
    let definition = port.definition()?;
    let authored = spec
        .ports
        .iter()
        .find(|candidate| std::ptr::eq(candidate.definition.as_ref(), definition))?;
    let TargetSpec::Reference(reference) = &authored.default_item else {
        return None;
    };
    let target = item.attachment_target()?.guid();
    (reference.guid() == target && target == item.guid())
        .then(|| port_field(definition, "defaultItem.entityClass"))
}

fn replacement_guid(item: &EntityNode) -> Option<PaintSelectionInput> {
    let target = item.attachment_target()?.guid();
    let record = item.record_index()?;
    (target == item.guid()).then(|| PaintSelectionInput::DataForge(record_header(record, "guid")))
}

pub(crate) fn record_header(record: u32, field: &str) -> SourceLocation {
    SourceLocation::Subresource {
        namespace: "dataforgeRecordHeader".into(),
        indices: vec![u64::from(record)],
        field: field.into(),
    }
}

fn replacement_tags(
    sources: &mut Vec<PaintSelectionInput>,
    root: &EntityNode,
    port: &PortNode,
    item: &EntityNode,
    paint: &PaintItem,
    offers: &PaintPortMatch<'_>,
) {
    let vehicle = root.vehicle();
    let xml = vehicle.and_then(|vehicle| vehicle.item_port(port.name()));
    push_port_tags(
        sources,
        port,
        xml.map(VehicleItemPort::required_tags_source),
        vehicle,
        "RequiredPortTags",
        PortDefinition::required_tags,
    );
    if !paint.required_tags.trim().is_empty() {
        sources.extend(attach_tags(item, "RequiredTags"));
    }
    if offers.uses_port_tags() {
        push_port_tags(
            sources,
            port,
            xml.map(VehicleItemPort::port_tags_source),
            vehicle,
            "PortTags",
            PortDefinition::port_tags,
        );
    }
    if offers.uses_vehicle_tags() {
        sources.extend(attach_tags(root, "Tags"));
    }
}

fn append_ancestry(
    sources: &mut Vec<PaintSelectionInput>,
    ancestry: &PaintTagAncestry<'_>,
) -> Result<(), &'static str> {
    let required = ancestry.required();
    for endpoint in [ancestry.offered(), required] {
        let record_index = endpoint
            .record_index()
            .ok_or("the matched Tag has no original Record")?;
        sources.push(PaintSelectionInput::DataForge(SourceLocation::DataForge {
            record_index,
            field: "tagName".to_owned(),
        }));
    }
    for child in ancestry.tags().take_while(|tag| *tag != required) {
        let record_index = child
            .parent()
            .and_then(|parent| parent.record_index())
            .ok_or("the matched Tag ancestry has no original parent Record")?;
        let ordinal = child
            .parent_child_ordinal()
            .ok_or("the selected Tag ancestry has no original children occurrence")?;
        sources.push(PaintSelectionInput::DataForge(SourceLocation::DataForge {
            record_index,
            field: format!("children[{ordinal}]"),
        }));
    }
    Ok(())
}

fn attach_tags(node: &EntityNode, field: &str) -> Option<PaintSelectionInput> {
    let (record_index, component) = node
        .record_index()
        .zip(node.attachable_component_ordinal())?;
    Some(PaintSelectionInput::DataForge(SourceLocation::DataForge {
        record_index,
        field: format!("Components[{component}].AttachDef.{field}"),
    }))
}

fn push_port_tags(
    sources: &mut Vec<PaintSelectionInput>,
    port: &PortNode,
    xml: Option<(Option<&str>, Option<&crate::VehicleAttributeEvidence>)>,
    vehicle: Option<&VehicleImplementation>,
    record_field: &str,
    value: impl FnOnce(&PortDefinition) -> &str,
) {
    if let Some((Some(_), evidence)) = xml {
        if let Some(evidence) = evidence {
            let route = vehicle.and_then(|vehicle| vehicle.explicit_patch_selector(evidence));
            for site in std::iter::once(evidence.original())
                .chain(evidence.selectors())
                .chain(route)
            {
                if let Some((entry, location)) =
                    site.file().source().entry_origin().zip(site.location())
                {
                    sources.push(PaintSelectionInput::Xml(entry.clone(), location.clone()));
                }
            }
        }
    } else if let Some(definition) = port
        .definition()
        .filter(|definition| !value(definition).trim().is_empty())
    {
        sources.push(port_field(definition, record_field));
    }
}

fn port_field(definition: &PortDefinition, field: &str) -> PaintSelectionInput {
    let source = definition.position();
    let component = source.component_ordinal();
    let port = source.port_ordinal();
    PaintSelectionInput::DataForge(SourceLocation::DataForge {
        record_index: source.record_index(),
        field: format!("Components[{component}].Ports[{port}].{field}"),
    })
}

#[cfg(test)]
#[path = "paint_tag_ancestry_tests.rs"]
mod tests;
