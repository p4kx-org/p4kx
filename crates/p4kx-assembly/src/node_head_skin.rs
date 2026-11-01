// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use crate::bespoke::{Bespoke, OwnSkin};
use crate::node_body::is_head_type;
use crate::{Attachment, EntityNode, Geometry, PortNode};

impl EntityNode {
    /// Gives the head of the character in the tree below this Entity, and the Entities on the
    /// head's ports, the own skin of a story character's head, see [`Bespoke`].
    ///
    /// The Assembly draws the skin when the Archive holds it, else the geometry the Entity had.
    /// A tree without a head or a named body, or a head whose library names no folder, is left as
    /// it is.
    pub(crate) fn wear_own_head_skins(&mut self) {
        if self.character_body().is_none() {
            return;
        }
        // `character_body` borrows the head shared; the skins are set through a mutable walk.
        let Some(head) = find_head_mut(&mut self.ports) else {
            return;
        };
        let library = head.geometry.as_ref().and_then(Geometry::material);
        let Some(own) = library.and_then(|library| Bespoke::of_head(head.class_name(), library))
        else {
            return;
        };
        let head_library: Arc<str> = Arc::from(own.library.as_str());
        wear(head, &own, &head_library);
        for port in &mut head.ports {
            if let Attachment::Entity(item) = &mut port.attachment {
                wear(item, &own, &head_library);
            }
        }
    }
}

/// Gives `node` the own skin of `own` for the generic head skin its geometry names, if it names
/// one.
fn wear(node: &mut EntityNode, own: &Bespoke, head_library: &Arc<str>) {
    let Some(current) = &node.geometry else {
        return;
    };
    let Some(path) = own.skin_for(current.path()) else {
        return;
    };
    node.own_skin = Some(OwnSkin {
        geometry: Geometry {
            path,
            ..current.clone()
        },
        head_library: Arc::clone(head_library),
    });
}

/// Returns the first Entity of the item type `Char_Head` below `ports`, depth first, like
/// `find_head`.
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
