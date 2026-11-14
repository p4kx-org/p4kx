// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::{PortDefinitionPosition, PortNode, PortSource, PortTransform};

/// One original port declaration used while placing an Assembly part.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum PortPlacementField {
    /// The attachment type selected the skin placement.
    AttachmentType,
    /// The port name selected a parent model node.
    PortName,
    /// The authored helper name selected the anchor.
    Helper,
    /// The authored translation used by the port offset.
    Position,
    /// The authored rotation used by the port offset.
    Rotation,
    /// The authored helper selected the child's inverse offset.
    ItemOffsetHelper,
}

impl PortPlacementField {
    /// Returns the original field relative to the physical port definition.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::AttachmentType => "AttachmentImplementation",
            Self::PortName => "Name",
            Self::Helper => "AttachmentImplementation.Helper.Helper.Name",
            Self::Position => "AttachmentImplementation.Helper.Helper.Offset.Position",
            Self::Rotation => "AttachmentImplementation.Helper.Helper.Offset.Rotation",
            Self::ItemOffsetHelper => "AttachmentImplementation.Helper.Helper.ItemOffsetHelperName",
        }
    }
}

/// Original authored inputs retained by an actual port placement.
#[derive(Debug, Clone)]
pub struct PortPlacementSource {
    port: PortDefinitionPosition,
    fields: [Option<PortPlacementField>; 5],
}

impl PortPlacementSource {
    /// Returns the original physical port declaration.
    #[must_use]
    pub const fn port(&self) -> PortDefinitionPosition {
        self.port
    }

    /// Returns the original fields whose values were used.
    pub fn fields(&self) -> impl Iterator<Item = PortPlacementField> + '_ {
        self.fields.iter().copied().flatten()
    }

    pub(crate) fn skin(port: &PortNode) -> Option<Self> {
        Some(Self {
            port: port.definition()?.position(),
            fields: [
                Some(PortPlacementField::AttachmentType),
                None,
                None,
                None,
                None,
            ],
        })
    }

    pub(crate) fn anchor(
        port: &PortNode,
        placement: &crate::Placement,
        transform: Option<&PortTransform>,
        offset_helper: bool,
    ) -> Option<Self> {
        let source = port.definition()?.position();
        let helper = matches!(placement, crate::Placement::Helper(_))
            && transform.is_some_and(|transform| !transform.helper().is_empty());
        let port_name = matches!(placement, crate::Placement::PortNode)
            && port.source() == PortSource::PortDefault;
        let [position, rotation] = transform.map_or([false; 2], PortTransform::offset_fields);
        let fields = [
            helper.then_some(PortPlacementField::Helper),
            port_name.then_some(PortPlacementField::PortName),
            position.then_some(PortPlacementField::Position),
            rotation.then_some(PortPlacementField::Rotation),
            offset_helper.then_some(PortPlacementField::ItemOffsetHelper),
        ];
        fields.iter().any(Option::is_some).then_some(Self {
            port: source,
            fields,
        })
    }
}
