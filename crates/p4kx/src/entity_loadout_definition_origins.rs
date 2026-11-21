// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use anyhow::Result;
use p4kx_assembly::{
    EntityNode, LoadoutError, PortNode, PortVisibility, VehicleImplementation, VehicleItemPort,
    VehiclePart,
};
use p4kx_extraction::{FieldOrigin, SourceLocation, SourceUseRole};

use super::Output;
use super::definition_sites::{DefinitionKey, Member};

#[derive(Clone, Copy)]
pub(super) enum VisibilityOwner<'node> {
    DataForge,
    Vehicle {
        implementation: &'node VehicleImplementation,
        selection: Option<&'node FieldOrigin>,
    },
    Unverified,
}

impl<'node> VisibilityOwner<'node> {
    pub(super) fn of(entity: &'node EntityNode, selection: Option<&'node FieldOrigin>) -> Self {
        if let Some(implementation) = entity.vehicle() {
            return Self::Vehicle {
                implementation,
                selection,
            };
        }
        if entity.issues().iter().any(|issue| {
            matches!(
                issue,
                LoadoutError::MissingVehicleImplementation { .. }
                    | LoadoutError::VehicleImplementation { .. }
            )
        }) {
            Self::Unverified
        } else {
            Self::DataForge
        }
    }
}

impl Output<'_, '_> {
    pub(super) fn definition(
        &mut self,
        port: &PortNode,
        address: &str,
        owner: VisibilityOwner<'_>,
    ) -> Result<()> {
        let definition_address = format!("{address}/definition");
        let name = self.path_origin(
            port.name_proof().filter(|path| path.value() == port.name()),
            SourceUseRole::Selector,
        )?;
        let facts = self.facts;
        let source = port.definition().and_then(|definition| {
            let key = DefinitionKey::of(definition.position());
            let captured = facts.definitions.get(&key)?;
            let [declaration, fields @ ..] = captured.members.as_slice() else {
                return None;
            };
            Some((key, declaration, fields))
        });
        let Some((key, declaration, fields)) = source else {
            let incomplete = self.uses.origin_with_origins([&name], false)?;
            self.put(&definition_address, incomplete)?;
            return self.definition_visibility(port, address, owner, None);
        };
        let declared = self.definition_member(key, declaration, SourceUseRole::Selector)?;
        let selected = self.uses.origin_with_origins([&name, &declared], true)?;
        self.put(&definition_address, selected.clone())?;
        let mut flags = None;
        for field in fields {
            let original = self.definition_member(key, field, SourceUseRole::Payload)?;
            let origin = self
                .uses
                .origin_with_origins([&selected, &original], true)?;
            self.put(&format!("{definition_address}{}", field.path), origin)?;
            if field.path == "/flags" && port.visibility() != PortVisibility::Unspecified {
                let original = self.definition_member(key, field, SourceUseRole::Conversion)?;
                flags = Some(
                    self.uses
                        .origin_with_origins([&selected, &original], true)?,
                );
            }
        }
        self.definition_visibility(port, address, owner, flags.as_ref())
    }

    fn definition_member(
        &self,
        key: DefinitionKey,
        member: &Member,
        payload_role: SourceUseRole,
    ) -> Result<FieldOrigin> {
        let mut source = Vec::new();
        for input in &member.inputs {
            let role = match input.role {
                SourceUseRole::Payload => payload_role,
                other => other,
            };
            let location = SourceLocation::Subresource {
                namespace: "dataforgeValue".into(),
                indices: vec![u64::from(key.record()), input.ordinal],
                field: "$value".into(),
            };
            source.push(self.forge_scope.at(location, role)?);
            if let Some(text) = &input.text {
                source.push(self.forge_scope.at(text.clone(), role)?);
            }
        }
        Ok(self
            .uses
            .origin(&source, member.complete && self.forge_complete))
    }

    fn definition_visibility(
        &mut self,
        node: &PortNode,
        address: &str,
        owner: VisibilityOwner<'_>,
        flags: Option<&FieldOrigin>,
    ) -> Result<()> {
        if node.visibility() == PortVisibility::Unspecified {
            return Ok(());
        }
        let origin = match owner {
            VisibilityOwner::DataForge => self.uses.origin_with_origins(flags, flags.is_some())?,
            VisibilityOwner::Vehicle {
                implementation,
                selection,
            } => match implementation.port_part(node.name()) {
                Some(part) => self.vehicle_visibility(node, part, selection)?,
                None => self
                    .uses
                    .origin_with_origins(flags.into_iter().chain(selection), false)?,
            },
            VisibilityOwner::Unverified => self.uses.origin_with_origins(flags, false)?,
        };
        self.put(&format!("{address}/visibility"), origin)
    }

    fn vehicle_visibility(
        &self,
        node: &PortNode,
        part: &VehiclePart,
        selection: Option<&FieldOrigin>,
    ) -> Result<FieldOrigin> {
        let mut inputs = vec![
            self.path_origin(
                node.name_proof()
                    .filter(|proof| proof.value() == node.name()),
                SourceUseRole::Selector,
            )?,
            self.path_origin(
                part.name_proof()
                    .filter(|proof| proof.value() == part.name()),
                SourceUseRole::Selector,
            )?,
        ];
        if let Some(evidence) = part.item_port().and_then(VehicleItemPort::flags_evidence) {
            let flags = evidence.original();
            inputs.push(self.xml_origin(
                flags.file(),
                flags.location(),
                SourceUseRole::Conversion,
            )?);
            for selector in evidence.selectors() {
                inputs.push(self.xml_origin(
                    selector.file(),
                    selector.location(),
                    SourceUseRole::Selector,
                )?);
            }
        }
        Ok(self
            .uses
            .origin_with_origins(inputs.iter().chain(selection), false)?)
    }
}
