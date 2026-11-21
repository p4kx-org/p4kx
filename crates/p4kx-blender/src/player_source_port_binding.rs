// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::ENTITY_TYPE;
use p4kx_assembly::character::lowered_hash;
use p4kx_extraction::{
    Guid, Record, SourceLocation, SourceScope, SourceUse, SourceUseRole, SourceUses, SourceValue,
    SourcedValue, StructValue, Value,
};
use schemars::JsonSchema;
use serde::Serialize;

use crate::player_source::PlayerSourceError;
use crate::player_source_classes::Classes;
use crate::player_source_port_helper::Attachment;
use crate::soc_scene_classes;

const PORT_CONTAINER: &str = "SItemPortContainerComponentParams";

pub(crate) struct OwnerPort<'a> {
    pub(crate) owner: Guid,
    pub(crate) crc: u32,
    pub(crate) owner_selector: &'a SourceUse,
    pub(crate) port_selector: &'a SourceUse,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub(crate) enum PortBinding {
    NoOwner,
    EmptyOwner,
    UndeclaredOwner,
    UnmatchedOwner,
    AmbiguousOwner { record_indices: Vec<u32> },
    UnexpectedOwnerType,
    UnmatchedPort { record_index: u32 },
    Matched { definition: Box<Definition> },
    AmbiguousPort { definitions: Vec<Definition> },
    IncompletePorts { definitions: Vec<Definition> },
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Definition {
    record_index: u32,
    component_ordinal: usize,
    port_ordinal: usize,
    name: SourcedValue<String>,
    flags: SourcedValue<String>,
    select_tag: SourcedValue<String>,
    attachment: SourcedValue<Attachment>,
}

pub(crate) fn read(
    sources: &SourceUses,
    classes: Classes<'_, '_>,
    port: Option<OwnerPort<'_>>,
    root: &SourceUse,
) -> Result<SourcedValue<PortBinding>, PlayerSourceError> {
    let Some(port) = port else {
        return Ok(sources.value(
            SourceValue::Present(PortBinding::NoOwner),
            std::slice::from_ref(root),
            true,
        ));
    };
    let mut uses = vec![port.port_selector.clone(), port.owner_selector.clone()];
    if port.owner == Guid::default() {
        return Ok(sources.value(SourceValue::Present(PortBinding::EmptyOwner), &uses, true));
    }
    let Some((entities, scope)) = classes else {
        return Ok(sources.value(SourceValue::Unreadable, &uses, false));
    };
    let records: Vec<_> = entities.forge().records_by_guid(port.owner).collect();
    selected(
        sources,
        scope,
        &records,
        PortName::Crc(port.crc),
        "$guid",
        &mut uses,
    )
}

pub(crate) fn named(
    sources: &SourceUses,
    classes: Classes<'_, '_>,
    owner: &str,
    name: &str,
    owner_selector: &SourceUse,
    port_selector: &SourceUse,
) -> Result<SourcedValue<PortBinding>, PlayerSourceError> {
    let mut uses = vec![port_selector.clone(), owner_selector.clone()];
    if owner.is_empty() {
        return Ok(sources.value(SourceValue::Present(PortBinding::EmptyOwner), &uses, true));
    }
    let Some((entities, scope)) = classes else {
        return Ok(sources.value(SourceValue::Unreadable, &uses, false));
    };
    let records = entities.named_class_candidates(owner);
    selected(
        sources,
        scope,
        records,
        PortName::Text(name),
        "$name",
        &mut uses,
    )
}

fn selected(
    sources: &SourceUses,
    scope: &SourceScope,
    records: &[Record<'_>],
    port: PortName<'_>,
    owner_field: &str,
    uses: &mut Vec<SourceUse>,
) -> Result<SourcedValue<PortBinding>, PlayerSourceError> {
    for record in records {
        uses.push(field(scope, record.index(), owner_field.into())?);
    }
    let value = match records {
        [] => {
            uses.push(scope.at(
                SourceLocation::Metadata {
                    field: "entityClassIndex".into(),
                },
                SourceUseRole::Payload,
            )?);
            SourceValue::Present(PortBinding::UnmatchedOwner)
        }
        [record] => definitions(record, port, sources, scope, uses)?,
        records => SourceValue::Present(PortBinding::AmbiguousOwner {
            record_indices: records.iter().map(Record::index).collect(),
        }),
    };
    let complete = !matches!(
        value,
        SourceValue::Unreadable | SourceValue::Present(PortBinding::IncompletePorts { .. })
    );
    Ok(sources.value(value, uses, complete))
}

#[derive(Clone, Copy)]
enum PortName<'a> {
    Crc(u32),
    Text(&'a str),
}

impl PortName<'_> {
    fn matches(self, name: &str) -> bool {
        match self {
            Self::Crc(crc) => lowered_hash(name) == crc,
            Self::Text(stored) => stored.eq_ignore_ascii_case(name),
        }
    }
}

fn definitions(
    record: &Record<'_>,
    port: PortName<'_>,
    sources: &SourceUses,
    scope: &SourceScope,
    uses: &mut Vec<SourceUse>,
) -> Result<SourceValue<PortBinding>, PlayerSourceError> {
    let record_index = record.index();
    uses.push(field(scope, record_index, "$type".into())?);
    match record.type_name() {
        Ok(ENTITY_TYPE) => {}
        Ok(_) => return Ok(SourceValue::Present(PortBinding::UnexpectedOwnerType)),
        Err(_) => return Ok(SourceValue::Unreadable),
    }
    let Ok(tree) = record.tree() else {
        return Ok(SourceValue::Unreadable);
    };
    uses.push(field(scope, record_index, "Components".into())?);
    let mut rows = Vec::new();
    let mut complete = true;
    if let Some(Value::Array(components)) = tree.field("Components") {
        for (ordinal, component) in components.iter().enumerate() {
            match component {
                Value::Struct(component) if component.type_name() == PORT_CONTAINER => {
                    let input = ComponentPorts {
                        sources,
                        scope,
                        record: record_index,
                        ordinal,
                        port,
                    };
                    complete &= input.read(component, &mut rows, uses)?;
                }
                Value::Error(_) => complete = false,
                _ => {}
            }
        }
    } else if matches!(tree.field("Components"), Some(Value::Error(_))) {
        complete = false;
    }
    if !complete {
        return Ok(SourceValue::Present(PortBinding::IncompletePorts {
            definitions: rows,
        }));
    }
    let binding = match rows.len() {
        0 => PortBinding::UnmatchedPort { record_index },
        1 => PortBinding::Matched {
            definition: Box::new(rows.pop().ok_or(PlayerSourceError::Binding)?),
        },
        _ => PortBinding::AmbiguousPort { definitions: rows },
    };
    Ok(SourceValue::Present(binding))
}

struct ComponentPorts<'a> {
    sources: &'a SourceUses,
    scope: &'a SourceScope,
    record: u32,
    ordinal: usize,
    port: PortName<'a>,
}

impl ComponentPorts<'_> {
    fn read(
        &self,
        component: &StructValue<'_>,
        rows: &mut Vec<Definition>,
        uses: &mut Vec<SourceUse>,
    ) -> Result<bool, PlayerSourceError> {
        let prefix = format!("Components[{}].Ports", self.ordinal);
        uses.push(field(self.scope, self.record, prefix.clone())?);
        let ports = match component.field("Ports") {
            Some(Value::Array(ports)) => ports,
            Some(Value::Error(_)) => return Ok(false),
            _ => return Ok(true),
        };
        let mut complete = true;
        for (ordinal, port) in ports.iter().enumerate() {
            match port {
                Value::Struct(port) => {
                    let name = port.field("Name");
                    if matches!(name, Some(Value::Error(_))) {
                        complete = false;
                    }
                    if let Some(Value::String(name)) = name
                        && self.port.matches(name)
                    {
                        let site = PortSite {
                            source: self,
                            prefix: format!("{prefix}[{ordinal}]"),
                        };
                        uses.push(site.used("Name")?);
                        rows.push(site.read(ordinal, port)?);
                    }
                }
                Value::Error(_) => complete = false,
                _ => {}
            }
        }
        Ok(complete)
    }
}

pub(crate) struct PortSite<'a> {
    source: &'a ComponentPorts<'a>,
    prefix: String,
}

impl PortSite<'_> {
    fn read(
        &self,
        port_ordinal: usize,
        port: &StructValue<'_>,
    ) -> Result<Definition, PlayerSourceError> {
        let extension = struct_member(port, "Extension");
        Ok(Definition {
            record_index: self.source.record,
            component_ordinal: self.source.ordinal,
            port_ordinal,
            name: self.cell("Name", soc_scene_classes::string(port.field("Name")))?,
            flags: self.cell("Flags", soc_scene_classes::string(port.field("Flags")))?,
            select_tag: self.cell(
                "Extension.SelectTag",
                soc_scene_classes::string(extension.and_then(|value| value.field("SelectTag"))),
            )?,
            attachment: Attachment::read(self, port.field("AttachmentImplementation"))?,
        })
    }

    pub(crate) fn cell<T>(
        &self,
        path: &str,
        value: SourceValue<T>,
    ) -> Result<SourcedValue<T>, PlayerSourceError> {
        let complete = !matches!(value, SourceValue::Unreadable);
        Ok(self
            .source
            .sources
            .value(value, &[self.used(path)?], complete))
    }

    fn used(&self, path: &str) -> Result<SourceUse, PlayerSourceError> {
        field(
            self.source.scope,
            self.source.record,
            format!("{}.{}", self.prefix, path),
        )
    }
}

fn field(
    scope: &SourceScope,
    record_index: u32,
    field: String,
) -> Result<SourceUse, PlayerSourceError> {
    Ok(scope.at(
        SourceLocation::DataForge {
            record_index,
            field,
        },
        SourceUseRole::Payload,
    )?)
}

pub(crate) fn struct_member<'value, 'source>(
    parent: &'value StructValue<'source>,
    name: &str,
) -> Option<&'value StructValue<'source>> {
    match parent.field(name) {
        Some(Value::Struct(value)) => Some(value),
        _ => None,
    }
}
