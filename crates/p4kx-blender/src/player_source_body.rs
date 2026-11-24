// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::BodyVariant;
use p4kx_assembly::character::{Chf, ItemPort};
use p4kx_extraction::{
    CancelToken, EntryOrigin, Guid, InputScope, PathIndex, PhysicalSourceRef, SourceInput,
    SourceLocation, SourceScope, SourceUse, SourceUseRole, SourceUses, SourceValue, SourcedValue,
};
use schemars::JsonSchema;
use serde::Serialize;

use crate::player_default_chf::{
    PlayerDefaultChf, PlayerDefaultChfPayloadError, read_player_default_chf,
};
use crate::player_source::{PlayerSourceError, check};
use crate::player_source_classes::{self, ClassTarget, Classes};
use crate::player_source_port_binding::{self, OwnerPort, PortBinding};

#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub(crate) enum Body {
    Missing {
        selector: &'static str,
    },
    Ambiguous {
        selector: &'static str,
        candidates: Vec<PhysicalSourceRef>,
    },
    Unreadable {
        source: PhysicalSourceRef,
    },
    InvalidChf {
        source: PhysicalSourceRef,
    },
    Present {
        source: PhysicalSourceRef,
        data: BodyData,
    },
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BodyData {
    body_guid: SourcedValue<String>,
    female_version: SourcedValue<u32>,
    male_version: SourcedValue<u32>,
    ports: SourcedValue<Vec<Port>>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct Port {
    parent: Option<usize>,
    ordinal: usize,
    #[serde(rename = "portCrc")]
    crc: SourcedValue<u32>,
    item_guid: SourcedValue<String>,
    item: SourcedValue<ClassTarget>,
    definition: SourcedValue<PortBinding>,
}

pub(crate) fn read(
    paths: &PathIndex<'_>,
    variant: BodyVariant,
    sources: &SourceUses,
    directory: &SourceUse,
    classes: Classes<'_, '_>,
    cancel: &CancelToken,
) -> Result<SourcedValue<Body>, PlayerSourceError> {
    let selected = read_player_default_chf(paths, variant, cancel)?;
    let mut uses = vec![directory.clone()];
    let body = match selected {
        PlayerDefaultChf::Missing { selector } => Body::Missing { selector },
        PlayerDefaultChf::Selected { origin, payload } => {
            project(&origin, &payload, sources, classes, &mut uses, cancel)?
        }
        PlayerDefaultChf::Ambiguous { selector, entries } => Body::Ambiguous {
            selector,
            candidates: entries
                .iter()
                .map(|entry| {
                    let observed = paths
                        .archive()
                        .observed_entry(entry)
                        .ok_or(PlayerSourceError::Binding)?;
                    let scope = sources.observe(
                        SourceInput::ArchiveView(observed),
                        InputScope::ArchiveDirectoryMetadata,
                    )?;
                    uses.push(scope.at(
                        SourceLocation::Metadata {
                            field: "$path".into(),
                        },
                        SourceUseRole::Reference,
                    )?);
                    Ok::<_, PlayerSourceError>(scope.source_ref().clone())
                })
                .collect::<Result<_, _>>()?,
        },
    };
    let complete = !matches!(body, Body::Unreadable { .. });
    Ok(sources.value(SourceValue::Present(body), &uses, complete))
}

fn project(
    origin: &EntryOrigin,
    payload: &Result<Box<Chf>, PlayerDefaultChfPayloadError>,
    sources: &SourceUses,
    classes: Classes<'_, '_>,
    uses: &mut Vec<SourceUse>,
    cancel: &CancelToken,
) -> Result<Body, PlayerSourceError> {
    let scope = sources.observe(SourceInput::Archive(origin), InputScope::Payload)?;
    let source = scope.source_ref().clone();
    if matches!(payload, Err(PlayerDefaultChfPayloadError::Unreadable(_))) {
        uses.push(at(&scope, &[], "$payload", SourceUseRole::Attempted)?);
        return Ok(Body::Unreadable { source });
    }
    uses.push(at(&scope, &[], "$payload", SourceUseRole::Payload)?);
    let Ok(chf) = payload else {
        return Ok(Body::InvalidChf { source });
    };
    Ok(Body::Present {
        source,
        data: BodyData::read(chf, sources, &scope, classes, cancel)?,
    })
}

impl BodyData {
    fn read(
        chf: &Chf,
        sources: &SourceUses,
        scope: &SourceScope,
        classes: Classes<'_, '_>,
        cancel: &CancelToken,
    ) -> Result<Self, PlayerSourceError> {
        let character = &chf.character;
        let field = |name| at(scope, &[], name, SourceUseRole::Payload);
        let ports = match &character.item_ports {
            Some(root) => SourceValue::Present(read_ports(root, sources, scope, classes, cancel)?),
            None => SourceValue::Absent,
        };
        Ok(Self {
            body_guid: sources.value(
                SourceValue::Present(character.body_type.to_string()),
                &[field("bodyType")?],
                true,
            ),
            female_version: sources.value(
                SourceValue::Present(character.versions.female),
                &[field("versions.female")?],
                true,
            ),
            male_version: sources.value(
                SourceValue::Present(character.versions.male),
                &[field("versions.male")?],
                true,
            ),
            ports: sources.value(ports, &[field("itemPorts")?], true),
        })
    }
}

fn read_ports(
    root: &ItemPort,
    sources: &SourceUses,
    scope: &SourceScope,
    classes: Classes<'_, '_>,
    cancel: &CancelToken,
) -> Result<Vec<Port>, PlayerSourceError> {
    let mut rows = Vec::new();
    let mut pending = vec![(None, 0, root)];
    let mut owner_selectors: Vec<(Guid, SourceUse)> = Vec::new();
    while let Some((parent, ordinal, port)) = pending.pop() {
        check(cancel)?;
        let index = rows.len();
        let site = [index as u64];
        let payload = |field| at(scope, &site, field, SourceUseRole::Payload);
        let selector = at(scope, &site, "item", SourceUseRole::Selector)?;
        let guid = Guid::from_stored(port.item.stored());
        let port_selector = at(scope, &site, "port", SourceUseRole::Selector)?;
        let owner =
            parent
                .and_then(|index| owner_selectors.get(index))
                .map(|(owner, owner_selector)| OwnerPort {
                    owner: *owner,
                    crc: port.port,
                    owner_selector,
                    port_selector: &port_selector,
                });
        let definition = player_source_port_binding::read(sources, classes, owner, &port_selector)?;
        rows.push(Port {
            parent,
            ordinal,
            crc: sources.value(SourceValue::Present(port.port), &[payload("port")?], true),
            item_guid: sources.value(
                SourceValue::Present(guid.to_string()),
                &[payload("item")?],
                true,
            ),
            item: player_source_classes::by_guid(sources, classes, &selector, guid)?,
            definition,
        });
        owner_selectors.push((guid, selector));
        pending.extend(
            port.children
                .iter()
                .enumerate()
                .rev()
                .map(|(ordinal, child)| (Some(index), ordinal, child)),
        );
    }
    Ok(rows)
}

fn at(
    scope: &SourceScope,
    indices: &[u64],
    field: &str,
    role: SourceUseRole,
) -> Result<SourceUse, PlayerSourceError> {
    Ok(scope.at(
        SourceLocation::Subresource {
            namespace: "chf".into(),
            indices: indices.to_vec(),
            field: field.into(),
        },
        role,
    )?)
}
