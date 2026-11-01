// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::{TestInstance, TestValue};

/// An item port definition of a synthetic Entity.
#[derive(Debug, Clone, PartialEq)]
pub struct TestPort {
    name: String,
    sizes: (u32, u32),
    types: Vec<(String, Vec<String>)>,
    helper: Option<(String, [f32; 3], [f32; 3])>,
    item_helper: String,
    skin: bool,
    default_item: Option<u128>,
    flags: String,
    tags: (String, String),
    detach_direction: Option<[f32; 3]>,
    entity_attachment: Option<bool>,
}

impl TestPort {
    /// Creates a port of size 1 without types, helper and default item.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            sizes: (1, 1),
            types: Vec::new(),
            helper: None,
            item_helper: String::new(),
            skin: false,
            default_item: None,
            flags: String::new(),
            tags: (String::new(), String::new()),
            detach_direction: None,
            entity_attachment: None,
        }
    }

    /// Sets the direction in which the item leaves the port when detached.
    #[must_use]
    pub const fn detach_direction(mut self, direction: [f32; 3]) -> Self {
        self.detach_direction = Some(direction);
        self
    }

    /// Gives the port an entity attachment whose constraint sets `noRotation` to `no_rotation`;
    /// it replaces a helper or skin attachment of the port.
    #[must_use]
    pub const fn entity_attachment(mut self, no_rotation: bool) -> Self {
        self.entity_attachment = Some(no_rotation);
        self
    }

    /// Sets the flags of the port, words separated by white space.
    #[must_use]
    pub fn flags(mut self, flags: impl Into<String>) -> Self {
        self.flags = flags.into();
        self
    }

    /// Sets the tags the port offers and the tags an item needs to fit it, words separated by
    /// white space.
    #[must_use]
    pub fn tags(mut self, port_tags: impl Into<String>, required: impl Into<String>) -> Self {
        self.tags = (port_tags.into(), required.into());
        self
    }

    /// Sets the smallest and largest item size the port takes.
    #[must_use]
    pub const fn sizes(mut self, min: u32, max: u32) -> Self {
        self.sizes = (min, max);
        self
    }

    /// Adds an item type the port takes, with its sub-types.
    #[must_use]
    pub fn item_type<S: Into<String>>(
        mut self,
        name: impl Into<String>,
        sub_types: impl IntoIterator<Item = S>,
    ) -> Self {
        self.types
            .push((name.into(), sub_types.into_iter().map(Into::into).collect()));
        self
    }

    /// Attaches the port to the bone helper `name`, offset by `position` and `rotation`.
    #[must_use]
    pub fn helper(
        mut self,
        name: impl Into<String>,
        position: [f32; 3],
        rotation: [f32; 3],
    ) -> Self {
        self.helper = Some((name.into(), position, rotation));
        self
    }

    /// Places the helper `name` of the attached Entity's own model at the port's helper.
    #[must_use]
    pub fn item_helper(mut self, name: impl Into<String>) -> Self {
        self.item_helper = name.into();
        self
    }

    /// Makes the attachment of the port a skin, with or without a helper.
    #[must_use]
    pub const fn skin(mut self) -> Self {
        self.skin = true;
        self
    }

    /// Sets the Entity the port holds when no Loadout names one.
    #[must_use]
    pub const fn default_item(mut self, guid: u128) -> Self {
        self.default_item = Some(guid);
        self
    }

    pub(crate) fn value(&self) -> TestValue {
        let mut instance = TestInstance::new("SItemPortDef")
            .value("Name", TestValue::text(self.name.clone()))
            .value("DisplayName", TestValue::text("@LOC_EMPTY"))
            .value("Flags", TestValue::text(self.flags.clone()))
            .value("PortTags", TestValue::text(self.tags.0.clone()))
            .value("RequiredPortTags", TestValue::text(self.tags.1.clone()))
            .value("MinSize", TestValue::UInt32(self.sizes.0))
            .value("MaxSize", TestValue::UInt32(self.sizes.1))
            .value(
                "Types",
                TestValue::Array(
                    self.types
                        .iter()
                        .map(|(name, sub_types)| {
                            TestValue::Struct(
                                TestInstance::new("SItemPortDefTypes")
                                    .value("Type", TestValue::text(name.clone()))
                                    .value(
                                        "SubTypes",
                                        TestValue::Array(
                                            sub_types
                                                .iter()
                                                .cloned()
                                                .map(TestValue::Text)
                                                .collect(),
                                        ),
                                    ),
                            )
                        })
                        .collect(),
                ),
            );
        if let Some(guid) = self.default_item {
            instance = instance.value(
                "defaultItem",
                TestValue::Struct(
                    TestInstance::new("SItemPortDefaultItemDef")
                        .value("entityClass", TestValue::Reference(guid)),
                ),
            );
        }
        if let Some(direction) = self.detach_direction {
            instance = instance.value("detachDirection", vec3(direction));
        }
        if let Some(no_rotation) = self.entity_attachment {
            instance = instance.value("AttachmentImplementation", entity_attachment(no_rotation));
        } else if self.helper.is_some() || self.skin {
            let kind = if self.skin { "Skin" } else { "Bone" };
            instance = instance.value(
                "AttachmentImplementation",
                attachment(kind, self.helper.as_ref(), &self.item_helper),
            );
        }
        TestValue::Struct(instance)
    }
}

fn vec3([x, y, z]: [f32; 3]) -> TestValue {
    TestValue::Struct(
        TestInstance::new("Vec3")
            .value("x", TestValue::Single(x))
            .value("y", TestValue::Single(y))
            .value("z", TestValue::Single(z)),
    )
}

fn entity_attachment(no_rotation: bool) -> TestValue {
    TestValue::Struct(
        TestInstance::new("SItemPortDefAttachmentImplementationEntity").value(
            "constraintParams",
            TestValue::Struct(
                TestInstance::new("SItemPortConstraintParams")
                    .value("noRotation", TestValue::Bool(no_rotation)),
            ),
        ),
    )
}

fn attachment(
    kind: &str,
    helper: Option<&(String, [f32; 3], [f32; 3])>,
    item_helper: &str,
) -> TestValue {
    let mut instance = TestInstance::new(format!("SItemPortDefAttachmentImplementation{kind}"));
    if let Some((name, position, rotation)) = helper {
        instance = instance.value(
            "Helper",
            helper_node(name, item_helper, *position, *rotation),
        );
    }
    TestValue::Struct(instance)
}

fn helper_node(name: &str, item_helper: &str, position: [f32; 3], rotation: [f32; 3]) -> TestValue {
    let xyz = |type_name: &str, [x, y, z]: [f32; 3]| {
        TestValue::Struct(
            TestInstance::new(type_name)
                .value("x", TestValue::Single(x))
                .value("y", TestValue::Single(y))
                .value("z", TestValue::Single(z)),
        )
    };
    let offset = TestInstance::new("QuatT")
        .value("Rotation", xyz("Ang3", rotation))
        .value("Position", xyz("Vec3", position));
    let helper = TestInstance::new("SItemPortDefHelper")
        .value("Name", TestValue::text(name))
        .value("ItemOffsetHelperName", TestValue::text(item_helper))
        .value("Offset", TestValue::Struct(offset));
    TestValue::Struct(
        TestInstance::new("SItemPortDefHelperNode").value("Helper", TestValue::Struct(helper)),
    )
}
