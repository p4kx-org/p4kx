// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_dataforge::StructValue;

use crate::read::{flag, struct_field, struct_items, text, texts, unsigned, xyz};

const ATTACHMENT_PREFIX: &str = "SItemPortDefAttachmentImplementation";
const SKIN_ATTACHMENT: &str = "Skin";

/// The definition of an item port in an Entity Record (`SItemPortDef`): what it takes and where
/// it sits.
#[derive(Debug, Clone, PartialEq)]
pub struct PortDefinition {
    pub(crate) display_name: String,
    pub(crate) flags: String,
    pub(crate) min_size: Option<u32>,
    pub(crate) max_size: Option<u32>,
    pub(crate) types: Vec<PortType>,
    pub(crate) port_tags: String,
    pub(crate) required_tags: String,
    pub(crate) transform: Option<PortTransform>,
    pub(crate) skin: bool,
    pub(crate) no_rotation: bool,
    pub(crate) detach_direction: [f32; 3],
}

impl PortDefinition {
    /// Returns whether the item keeps its own orientation and takes only the translation of the
    /// port (`constraintParams.noRotation` of an entity attachment), as gimbals and turrets do.
    #[must_use]
    pub const fn no_rotation(&self) -> bool {
        self.no_rotation
    }

    /// Returns the direction in which the item leaves the port when detached, `[x, y, z]` as
    /// stored.
    #[must_use]
    pub const fn detach_direction(&self) -> [f32; 3] {
        self.detach_direction
    }

    /// Returns the tags the port offers, as stored: words separated by white space.
    #[must_use]
    pub fn port_tags(&self) -> &str {
        &self.port_tags
    }

    /// Returns the tags an item must carry to fit the port, as stored: words separated by white
    /// space.
    #[must_use]
    pub fn required_tags(&self) -> &str {
        &self.required_tags
    }

    /// Returns the display name, a localisation key such as `@LOC_EMPTY`.
    #[must_use]
    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    /// Returns the flags as stored, e.g. `invisible uneditable`.
    #[must_use]
    pub fn flags(&self) -> &str {
        &self.flags
    }

    /// Returns the smallest item size the port takes, or `None` when the Record gives none.
    #[must_use]
    pub const fn min_size(&self) -> Option<u32> {
        self.min_size
    }

    /// Returns the largest item size the port takes, or `None` when the Record gives none.
    #[must_use]
    pub const fn max_size(&self) -> Option<u32> {
        self.max_size
    }

    /// Returns the item types the port takes, in stored order.
    #[must_use]
    pub fn types(&self) -> &[PortType] {
        &self.types
    }

    /// Returns whether the item is a skin, drawn in the parent's bind pose whatever the helper and
    /// the offset say; independent of whether the Record gives a helper.
    #[must_use]
    pub const fn is_skin(&self) -> bool {
        self.skin
    }

    /// Returns where the port sits, or `None` when the Record gives no attachment helper.
    #[must_use]
    pub const fn transform(&self) -> Option<&PortTransform> {
        self.transform.as_ref()
    }
}

/// One item type a port takes, e.g. `WeaponGun` with the sub-type `Gun`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortType {
    pub(crate) name: String,
    pub(crate) sub_types: Vec<String>,
}

impl PortType {
    /// Returns the type name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the sub-types; empty when the port takes every sub-type.
    #[must_use]
    pub fn sub_types(&self) -> &[String] {
        &self.sub_types
    }
}

/// Where a port sits: the helper of the parent's geometry it attaches to and the offset from it.
#[derive(Debug, Clone, PartialEq)]
pub struct PortTransform {
    pub(crate) attachment: String,
    pub(crate) helper: String,
    pub(crate) offset_helper: String,
    pub(crate) position: [f32; 3],
    pub(crate) rotation: [f32; 3],
}

impl PortTransform {
    /// Returns how the item attaches, e.g. `Bone`, `Entity`, `Skin` or `StatObj`.
    #[must_use]
    pub fn attachment(&self) -> &str {
        &self.attachment
    }

    /// Returns the name of the helper in the parent's geometry; empty when it gives none.
    #[must_use]
    pub fn helper(&self) -> &str {
        &self.helper
    }

    /// Returns the name of the helper in the item's geometry placed on the port; usually empty.
    #[must_use]
    pub fn offset_helper(&self) -> &str {
        &self.offset_helper
    }

    /// Returns the offset from the helper in metres, `[x, y, z]`.
    #[must_use]
    pub const fn position(&self) -> [f32; 3] {
        self.position
    }

    /// Returns the rotation from the helper in degrees as stored, `[x, y, z]`.
    #[must_use]
    pub const fn rotation(&self) -> [f32; 3] {
        self.rotation
    }
}

/// Reads one `SItemPortDef`.
pub(crate) fn read_definition(port: &StructValue<'_>) -> PortDefinition {
    let attachment = struct_field(port, "AttachmentImplementation");
    PortDefinition {
        display_name: text(port, "DisplayName").to_owned(),
        flags: text(port, "Flags").to_owned(),
        min_size: unsigned(port, "MinSize"),
        max_size: unsigned(port, "MaxSize"),
        types: struct_items(port, "Types")
            .map(|item_type| PortType {
                name: text(item_type, "Type").to_owned(),
                sub_types: texts(item_type, "SubTypes"),
            })
            .collect(),
        port_tags: text(port, "PortTags").to_owned(),
        required_tags: text(port, "RequiredPortTags").to_owned(),
        transform: attachment.and_then(read_transform),
        skin: attachment.is_some_and(|attachment| attachment_kind(attachment) == SKIN_ATTACHMENT),
        no_rotation: attachment
            .and_then(|attachment| struct_field(attachment, "constraintParams"))
            .is_some_and(|constraints| flag(constraints, "noRotation")),
        detach_direction: xyz(struct_field(port, "detachDirection")),
    }
}

fn attachment_kind<'a>(attachment: &StructValue<'a>) -> &'a str {
    let kind = attachment.type_name();
    kind.strip_prefix(ATTACHMENT_PREFIX).unwrap_or(kind)
}

fn read_transform(attachment: &StructValue<'_>) -> Option<PortTransform> {
    let helper =
        struct_field(attachment, "Helper").and_then(|node| struct_field(node, "Helper"))?;
    let offset = struct_field(helper, "Offset");
    Some(PortTransform {
        attachment: attachment_kind(attachment).to_owned(),
        helper: text(helper, "Name").to_owned(),
        offset_helper: text(helper, "ItemOffsetHelperName").to_owned(),
        position: xyz(offset.and_then(|offset| struct_field(offset, "Position"))),
        rotation: xyz(offset.and_then(|offset| struct_field(offset, "Rotation"))),
    })
}
