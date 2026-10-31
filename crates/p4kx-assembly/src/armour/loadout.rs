// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_dataforge::Guid;

use super::piece::{Mount, Note};
use super::{Piece, Wearable, ZoneList};
use crate::resolver::LoadoutResolver;
use crate::spec::{EntitySpec, LoadoutSpec};
use crate::walk::Walk;
use crate::{Attachment, EntityNode, LoadoutError, PortNode, Target};

const WORN_PREFIXES: [&str; 2] = ["Char_Armor_", "Char_Clothing_"];

/// What the ports of a Loadout level belong to.
#[derive(Clone, Copy)]
enum Holder<'a> {
    /// The character: the root of the tree.
    Body,
    /// A worn piece of this name.
    Piece(&'a str),
    /// An Entity that is not worn, whose geometry is not drawn.
    Other,
}

impl Holder<'_> {
    /// Returns where an item on `port` hangs on this holder, or `None` when the holder is not
    /// drawn, the port is a skin or names no helper.
    fn mount(self, port: &PortNode) -> Option<Mount> {
        let holder = match self {
            Self::Body => None,
            Self::Piece(name) => Some(name.to_owned()),
            Self::Other => return None,
        };
        let definition = port.definition()?;
        let transform = definition.transform()?;
        (!definition.is_skin() && !transform.helper().is_empty())
            .then(|| Mount::new(holder, transform))
    }
}

/// Returns whether an Entity of the `AttachDef.Type` `item_type` is armour or clothing worn on a
/// body, ignoring ASCII case.
#[must_use]
pub fn is_worn_type(item_type: &str) -> bool {
    WORN_PREFIXES.iter().any(|prefix| {
        item_type
            .get(..prefix.len())
            .is_some_and(|start| start.eq_ignore_ascii_case(prefix))
    })
}

/// A piece of a Loadout that a piece given on top of it replaced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Replaced {
    pub(crate) name: String,
    pub(crate) by: String,
    pub(crate) item_type: String,
}

impl Replaced {
    /// Returns the name of the piece that was left out.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the name of the piece that took its place.
    #[must_use]
    pub fn by(&self) -> &str {
        &self.by
    }

    /// Returns the item type both pieces have.
    #[must_use]
    pub fn item_type(&self) -> &str {
        &self.item_type
    }
}

impl Piece {
    /// Returns the `AttachDef.Type` of the Entity the piece wears, if it comes from one and the
    /// Record names a type.
    #[must_use]
    pub fn item_type(&self) -> Option<&str> {
        match &self.source {
            super::piece::Source::Wearable(wearable) => {
                Some(wearable.item_type()).filter(|item_type| !item_type.is_empty())
            }
            _ => None,
        }
    }

    /// Puts `over` on top of the pieces of a Loadout: a piece of `over` with an item type takes
    /// the place of the pieces of `base` with the same item type, ignoring ASCII case, and every
    /// other piece of `over` is added. Returns the pieces, `base` first, and what was replaced.
    #[must_use]
    pub fn layer(base: Vec<Self>, over: Vec<Self>) -> (Vec<Self>, Vec<Replaced>) {
        let mut replaced = Vec::new();
        let mut pieces = Vec::with_capacity(base.len().saturating_add(over.len()));
        for piece in base {
            let taken_by = piece.item_type().and_then(|item_type| {
                over.iter().find(|other| {
                    other
                        .item_type()
                        .is_some_and(|other| other.eq_ignore_ascii_case(item_type))
                })
            });
            if let (Some(by), Some(item_type)) = (taken_by, piece.item_type()) {
                replaced.push(Replaced {
                    name: piece.name.clone(),
                    by: by.name.clone(),
                    item_type: item_type.to_owned(),
                });
            } else {
                pieces.push(piece);
            }
        }
        pieces.extend(over);
        (pieces, replaced)
    }
}

impl<'f> LoadoutResolver<'f> {
    /// Returns the paths of the XML Loadouts the default Loadout of `entity` is made of, in the
    /// order they are laid over each other: for an outfit its base Loadout, then the Loadouts of
    /// its outfits. Empty when the Entity's Loadout is not made of XML files or it has none.
    pub fn loadout_files(&mut self, entity: &p4kx_dataforge::Record<'f>) -> Vec<String> {
        let spec = self.spec(entity.guid());
        let mut paths = Vec::new();
        if let Ok(spec) = spec.as_ref()
            && let Some(loadout) = &spec.loadout
        {
            loadout.xml_paths(&mut paths);
        }
        paths
    }

    /// Resolves the XML Loadout at `path`, a path as a Record names it, into a tree whose root
    /// is no Entity: its ports are the Loadout's entries, with the Loadouts it includes merged.
    ///
    /// # Errors
    ///
    /// Returns the reason when no files were given to read it from or it cannot be read.
    pub fn resolve_file(&mut self, path: &str) -> Result<EntityNode, LoadoutError> {
        let loadout = LoadoutSpec::Xml(path.into());
        self.entries(Some(&loadout))?;
        let spec = EntitySpec {
            guid: Guid::default(),
            record_path: path.to_owned(),
            origin: crate::spec::LoadoutOrigin::File(path.into()),
            name: path.to_owned(),
            ports: Vec::new(),
            port_flags: Vec::new(),
            port_index: std::collections::HashMap::new(),
            loadout: Some(loadout),
            outfit: None,
            geometry: None,
            variants: Vec::new(),
            vehicle: None,
            docked_vehicle: false,
            attach_type: String::new(),
            attach_sub_type: String::new(),
            attach_tags: Vec::new(),
            material_variants: Vec::new(),
            size: None,
            container: None,
            object_containers: Vec::new().into(),
            zones: ZoneList::default(),
            animation_database: None,
        };
        let mut walk = Walk::new(spec.guid, None);
        Ok(self.node(&spec, None, &mut walk))
    }

    /// Returns the pieces of armour and clothing a Loadout tree holds, in tree order: every
    /// Entity of an item type that [`is_worn_type`] accepts, wherever it hangs in the tree, named
    /// `<port>/<class name>`.
    ///
    /// What cannot be resolved stays a [`Piece::failed`] so that the report lists it: an
    /// attachment that fails, an Entity whose Record cannot be read, and what the Loadout of the
    /// root or of a worn Entity could not read. Failed pieces are named `<port>/<class name>`,
    /// `<port>/<GUID>` or, when the Reference cannot be read, `<port>/unreadable`. Nothing below
    /// an Entity that is not worn, such as a weapon, is listed as failed, and other Entities are
    /// not pieces.
    pub fn worn(&mut self, tree: &EntityNode) -> Vec<Piece> {
        let mut pieces: Vec<Piece> = failures(tree.issues(), tree.record_path()).collect();
        self.collect_worn(tree.ports(), (true, Holder::Body), &mut pieces);
        pieces
    }

    fn collect_worn(
        &mut self,
        ports: &[PortNode],
        (relevant, holder): (bool, Holder<'_>),
        pieces: &mut Vec<Piece>,
    ) {
        for port in ports {
            let mut below = None;
            match port.attachment() {
                Attachment::Entity(node) => {
                    let name = is_worn_type(node.attach_type())
                        .then(|| format!("{}/{}", port.name(), node.class_name()));
                    if let Some(name) = &name {
                        let spec = self.spec(node.guid());
                        let piece = match spec.as_ref() {
                            Ok(spec) => relevant_issues(node.issues()).fold(
                                Piece::wearable(Wearable::from_spec(spec)).named(name.clone()),
                                |piece, issue| {
                                    piece.noting(Note::LoadoutUnreadable(issue.to_string()))
                                },
                            ),
                            Err(error) => Piece::failed(name.clone(), error.to_string()),
                        };
                        let piece = piece.at_port(port.name());
                        pieces.push(match holder.mount(port) {
                            Some(mount) => piece.mounted(mount),
                            None => piece,
                        });
                    }
                    below = Some((node, name));
                }
                Attachment::Unresolved(unresolved) if relevant => {
                    let named = match unresolved.target() {
                        Target::ClassName(class) => class.clone(),
                        Target::Reference(guid) => guid.to_string(),
                        Target::Unreadable => "unreadable".to_owned(),
                    };
                    let name = format!("{}/{named}", port.name());
                    pieces.push(Piece::failed(name, unresolved.error().to_string()));
                }
                _ => {}
            }
            self.collect_worn(port.nested(), (relevant, holder), pieces);
            if let Some((node, name)) = &below {
                let inside = name.as_deref().map_or(Holder::Other, Holder::Piece);
                self.collect_worn(node.ports(), (name.is_some(), inside), pieces);
            }
        }
    }
}

fn relevant_issues(issues: &[LoadoutError]) -> impl Iterator<Item = &LoadoutError> {
    issues.iter().filter(|issue| {
        !matches!(
            issue,
            LoadoutError::AmbiguousClass { .. } | LoadoutError::VehicleImplementation { .. }
        )
    })
}

fn failures<'a>(
    issues: &'a [LoadoutError],
    name: &'a str,
) -> impl Iterator<Item = Piece> + use<'a> {
    relevant_issues(issues).map(move |issue| Piece::failed(name, issue.to_string()))
}
