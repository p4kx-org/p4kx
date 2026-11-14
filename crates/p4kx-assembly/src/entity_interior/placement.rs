// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_model::{Model, Transform};

use super::EntityInterior;
use super::lookup::near;
use crate::placement::offset_transform;
use crate::{ContainerSource, IssueKind, ObjectContainerRef, RoomModelHelper};

pub(super) struct ContainerPlacement {
    pub(super) transform: Transform,
    pub(super) tile_owner: Option<Transform>,
    pub(super) helper_node: Option<usize>,
}

impl EntityInterior {
    pub(super) fn place(
        &mut self,
        at: &str,
        reference: &ObjectContainerRef,
        model: Option<&Model>,
    ) -> ContainerPlacement {
        let finite = reference
            .position()
            .iter()
            .chain(&reference.rotation())
            .all(|value| value.is_finite());
        let moved = if finite {
            offset_transform(reference.position(), reference.rotation())
        } else {
            self.note(
                at,
                reference,
                IssueKind::InvalidOffset {
                    name: reference.file().to_owned(),
                },
            );
            Transform::IDENTITY
        };
        let bone = reference.bone();
        let proven = finite && reference.source() == ContainerSource::Vehicle;
        if bone.is_empty() {
            return ContainerPlacement::new(moved, proven, None);
        }
        let Some((model, helper)) = model
            .and_then(|model| RoomModelHelper::find(model, bone).map(|helper| (model, helper)))
        else {
            self.note(
                at,
                reference,
                IssueKind::HelperNotFound {
                    helper: bone.to_owned(),
                },
            );
            return ContainerPlacement::new(moved, false, None);
        };
        let unique = model
            .nodes
            .iter()
            .filter(|node| node.name.eq_ignore_ascii_case(bone))
            .nth(1)
            .is_none();
        let anchor = helper.transform();
        if helper.uses_offset() {
            return ContainerPlacement::new(
                anchor.then(&moved),
                proven && unique,
                Some(helper.node()),
            );
        }
        self.check_position_offset(at, reference, anchor, finite);
        ContainerPlacement::new(anchor, proven && unique, Some(helper.node()))
    }
    fn check_position_offset(
        &mut self,
        at: &str,
        reference: &ObjectContainerRef,
        anchor: Transform,
        finite: bool,
    ) {
        if finite
            && !near(reference.position(), [0.0; 3])
            && !near(reference.position(), anchor.translation())
        {
            self.note(
                at,
                reference,
                IssueKind::OffsetDiffers {
                    bone: reference.bone().to_owned(),
                },
            );
        }
    }
}

impl ContainerPlacement {
    fn new(transform: Transform, proven: bool, helper_node: Option<usize>) -> Self {
        Self {
            transform,
            tile_owner: proven.then_some(transform),
            helper_node: helper_node.filter(|_| proven),
        }
    }
}
