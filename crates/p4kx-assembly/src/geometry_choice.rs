// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::BodyVariant;

/// How the geometry of an Entity in a Loadout tree was chosen.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum GeometryChoice {
    /// The Record's main geometry, nothing chose another.
    #[default]
    Main,
    /// The flags of the port the Entity sits at chose a sub-geometry.
    Flag,
    /// The body of the character wearing the Entity chose a sub-geometry.
    Body(BodyVariant),
    /// The head of a story character has its own skin here, see
    /// `EntityNode::wear_own_head_skins`; `fallback` holds the generic one.
    Bespoke,
    /// The Entity is worn, has no sub-geometry per body and names only a world prop; the prop
    /// is not drawn and `fallback` holds it.
    WorldProp,
}
