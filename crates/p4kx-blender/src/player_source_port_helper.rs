// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValue, SourcedValue, StructValue, Value};
use schemars::JsonSchema;
use serde::Serialize;

use crate::player_source::PlayerSourceError;
use crate::player_source_port_binding::{PortSite, struct_member};
use crate::soc_scene_classes;

const IMPLEMENTATION: &str = "AttachmentImplementation";
const HELPER_NODE: &str = "AttachmentImplementation.Helper";

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Attachment {
    declared_type: SourcedValue<String>,
    helpers: SourcedValue<Vec<Helper>>,
    no_rotation: SourcedValue<bool>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct Helper {
    parent: Option<usize>,
    ordinal: usize,
    tags: SourcedValue<String>,
    subhelper_count: SourcedValue<usize>,
    name: SourcedValue<String>,
    item_offset_helper_name: SourcedValue<String>,
    position: SourcedValue<Vector>,
    rotation: SourcedValue<Vector>,
}

#[derive(Debug, Serialize, JsonSchema)]
struct Vector {
    x: SourcedValue<f64>,
    y: SourcedValue<f64>,
    z: SourcedValue<f64>,
}

impl Attachment {
    pub(crate) fn read(
        site: &PortSite<'_>,
        value: Option<&Value<'_>>,
    ) -> Result<SourcedValue<Self>, PlayerSourceError> {
        let value = match value {
            Some(Value::Struct(implementation)) => {
                let declared_type = site.cell(
                    IMPLEMENTATION,
                    SourceValue::Present(implementation.type_name().to_owned()),
                )?;
                let helpers = state(implementation.field("Helper"), |root| {
                    read_helpers(site, root)
                })?;
                SourceValue::Present(Self {
                    declared_type,
                    helpers: site.cell(HELPER_NODE, helpers)?,
                    no_rotation: site.cell(
                        "AttachmentImplementation.constraintParams.noRotation",
                        no_rotation(implementation),
                    )?,
                })
            }
            Some(Value::Null) => SourceValue::Null,
            Some(Value::Error(_)) => SourceValue::Unreadable,
            None => SourceValue::Absent,
            _ => SourceValue::Invalid,
        };
        site.cell(IMPLEMENTATION, value)
    }
}

fn read_helpers(
    site: &PortSite<'_>,
    root: &StructValue<'_>,
) -> Result<Vec<Helper>, PlayerSourceError> {
    let mut rows = Vec::new();
    let mut pending = vec![(None, 0, HELPER_NODE.to_owned(), root)];
    while let Some((parent, ordinal, path, node)) = pending.pop() {
        let index = rows.len();
        if let Some(Value::Array(children)) = node.field("SubHelpers") {
            pending.extend(
                children
                    .iter()
                    .enumerate()
                    .rev()
                    .filter_map(|(ordinal, child)| {
                        let Value::Struct(child) = child else {
                            return None;
                        };
                        Some((
                            Some(index),
                            ordinal,
                            format!("{path}.SubHelpers[{ordinal}]"),
                            child,
                        ))
                    }),
            );
        }
        rows.push(Helper::read(site, node, parent, ordinal, &path)?);
    }
    Ok(rows)
}

impl Helper {
    fn read(
        site: &PortSite<'_>,
        node: &StructValue<'_>,
        parent: Option<usize>,
        ordinal: usize,
        path: &str,
    ) -> Result<Self, PlayerSourceError> {
        let helper = struct_member(node, "Helper");
        let offset = helper.and_then(|helper| struct_member(helper, "Offset"));
        let translation = format!("{path}.Helper.Offset.Position");
        let rotation = format!("{path}.Helper.Offset.Rotation");
        let text = |field, owner: Option<&StructValue<'_>>, source| {
            site.cell(
                field,
                soc_scene_classes::string(owner.and_then(|value| value.field(source))),
            )
        };
        Ok(Self {
            parent,
            ordinal,
            tags: text(&format!("{path}.Tags"), Some(node), "Tags")?,
            subhelper_count: site.cell(
                &format!("{path}.SubHelpers"),
                match node.field("SubHelpers") {
                    Some(Value::Array(children)) => SourceValue::Present(children.len()),
                    Some(Value::Null) => SourceValue::Null,
                    Some(Value::Error(_)) => SourceValue::Unreadable,
                    None => SourceValue::Absent,
                    _ => SourceValue::Invalid,
                },
            )?,
            name: text(&format!("{path}.Helper.Name"), helper, "Name")?,
            item_offset_helper_name: text(
                &format!("{path}.Helper.ItemOffsetHelperName"),
                helper,
                "ItemOffsetHelperName",
            )?,
            position: vector(
                site,
                &translation,
                offset.and_then(|value| value.field("Position")),
            )?,
            rotation: vector(
                site,
                &rotation,
                offset.and_then(|value| value.field("Rotation")),
            )?,
        })
    }
}

fn vector(
    site: &PortSite<'_>,
    path: &str,
    value: Option<&Value<'_>>,
) -> Result<SourcedValue<Vector>, PlayerSourceError> {
    let value = state(value, |vector| {
        let axis = |name| site.cell(&format!("{path}.{name}"), number(vector.field(name)));
        Ok(Vector {
            x: axis("x")?,
            y: axis("y")?,
            z: axis("z")?,
        })
    })?;
    site.cell(path, value)
}

fn state<T>(
    value: Option<&Value<'_>>,
    project: impl FnOnce(&StructValue<'_>) -> Result<T, PlayerSourceError>,
) -> Result<SourceValue<T>, PlayerSourceError> {
    Ok(match value {
        Some(Value::Struct(value)) => SourceValue::Present(project(value)?),
        None => SourceValue::Absent,
        Some(Value::Null) => SourceValue::Null,
        Some(Value::Error(_)) => SourceValue::Unreadable,
        _ => SourceValue::Invalid,
    })
}

fn number(value: Option<&Value<'_>>) -> SourceValue<f64> {
    match value {
        Some(Value::Single(number)) if number.is_finite() => {
            SourceValue::Present(f64::from(*number))
        }
        Some(Value::Double(number)) if number.is_finite() => SourceValue::Present(*number),
        None => SourceValue::Absent,
        Some(Value::Null) => SourceValue::Null,
        Some(Value::Error(_)) => SourceValue::Unreadable,
        _ => SourceValue::Invalid,
    }
}

fn no_rotation(implementation: &StructValue<'_>) -> SourceValue<bool> {
    match implementation.field("constraintParams") {
        Some(Value::Struct(constraints)) => match constraints.field("noRotation") {
            Some(Value::Bool(value)) => SourceValue::Present(*value),
            Some(Value::Null) => SourceValue::Null,
            Some(Value::Error(_)) => SourceValue::Unreadable,
            None => SourceValue::Absent,
            _ => SourceValue::Invalid,
        },
        Some(Value::Null) => SourceValue::Null,
        Some(Value::Error(_)) => SourceValue::Unreadable,
        None => SourceValue::Absent,
        _ => SourceValue::Invalid,
    }
}
