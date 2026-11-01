// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use crate::bespoke::Bespoke;
use crate::geometry_choice::GeometryChoice;
use crate::node_body::is_head_type;
use crate::{Attachment, EntityNode, Geometry, PortNode};

impl EntityNode {
    /// Gives the head of the character in the tree below this Entity, and the Entities on the
    /// head's ports, the own skins of a story character's head, see [`Bespoke`]: a part whose
    /// geometry is a generic head skin takes `<prefix>_<part>.skin` from the folder of the head's
    /// material library. The generic skin stays as the fallback, for an Archive without the
    /// file.
    ///
    /// A tree whose character has no head, or a head whose library names no folder, is left as it
    /// is.
    pub(crate) fn wear_own_head_skins(&mut self) {
        if self.character_body().is_none() {
            return;
        }
        let Some(head) = find_head_mut(&mut self.ports) else {
            return;
        };
        let library = head.geometry.as_ref().and_then(Geometry::material);
        let Some(own) = Bespoke::of_head(head.class_name(), library) else {
            return;
        };
        wear(head, &own);
        for port in &mut head.ports {
            if let Attachment::Entity(item) = &mut port.attachment {
                wear(item, &own);
            }
        }
    }
}

/// Takes the own skin of `own` for the generic head skin the geometry of `node` names, if it
/// names one.
fn wear(node: &mut EntityNode, own: &Bespoke) {
    let Some(current) = &node.geometry else {
        return;
    };
    let Some(path) = own.skin_for(current.path()) else {
        return;
    };
    let own_skin = Geometry {
        path,
        ..current.clone()
    };
    node.choice = GeometryChoice::Bespoke;
    node.head_library = own.library.as_deref().map(Arc::from);
    node.fallback = node.geometry.replace(own_skin);
}

/// Returns the first Entity of the item type `Char_Head` below `ports`, depth first.
fn find_head_mut(ports: &mut [PortNode]) -> Option<&mut EntityNode> {
    for port in ports {
        if let Attachment::Entity(node) = &mut port.attachment {
            if is_head_type(node.attach_type()) {
                return Some(&mut **node);
            }
            if let Some(found) = find_head_mut(&mut node.ports) {
                return Some(found);
            }
        }
        if let Some(found) = find_head_mut(&mut port.nested) {
            return Some(found);
        }
    }
    None
}
