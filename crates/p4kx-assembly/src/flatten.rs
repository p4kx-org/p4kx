// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::{
    Attachment, EntityNode, Geometry, Issue, IssueKind, LandingGear, MAX_ASSEMBLY_DEPTH, PortNode,
    PortVisibility, Target,
};

/// One part of an Assembly before its model is read: an Entity of the Loadout tree with its port,
/// or one landing gear of an Entity, whose `node` is that Entity and whose `geometry` is `None`.
pub(crate) struct Draft<'t> {
    pub(crate) name: String,
    pub(crate) path: String,
    pub(crate) port: Option<&'t PortNode>,
    pub(crate) gear: Option<&'t LandingGear>,
    pub(crate) node: &'t EntityNode,
    pub(crate) parent: Option<usize>,
    pub(crate) geometry: Option<&'t Geometry>,
    pub(crate) hidden: bool,
}

impl Draft<'_> {
    /// Returns the class name of the Entity part, or the bone of a landing gear.
    pub(crate) fn class_name(&self) -> &str {
        self.gear
            .map_or_else(|| self.node.class_name(), LandingGear::bone)
    }

    /// Returns whether the part's Entity names object containers of its own: a landing gear does
    /// not, nor does a hidden part, nor a docked vehicle that is not expanded, as its ports are
    /// left out too.
    pub(crate) const fn names_containers(&self) -> bool {
        self.gear.is_none()
            && !self.hidden
            && (self.node.is_expanded() || !self.node.is_docked_vehicle())
    }

    /// Returns the key the part's model is kept under: the file, and for a landing gear a key of
    /// its own, as the gear is posed and an Entity that names the same file is not.
    pub(crate) fn model_key(&self) -> Option<String> {
        let key = crate::build::file_key(self.geometry_path()?);
        Some(if self.gear.is_some() {
            format!("gear\\{key}")
        } else {
            key
        })
    }

    /// Returns the path of the model file the part draws.
    pub(crate) fn geometry_path(&self) -> Option<&str> {
        self.gear.map_or_else(
            || self.geometry.map(Geometry::path),
            |gear| Some(gear.geometry()).filter(|path| !path.is_empty()),
        )
    }
}

/// The parts of a Loadout tree in order, every part after its parent, and what could not be
/// turned into one.
pub(crate) struct Flat<'t> {
    pub(crate) drafts: Vec<Draft<'t>>,
    pub(crate) issues: Vec<Issue>,
    pub(crate) unresolved: Vec<UnresolvedPort<'t>>,
}

/// An attachment that could not be resolved: its issue, the draft whose Loadout holds it and the
/// name of its port.
#[derive(Clone, Copy)]
pub(crate) struct UnresolvedPort<'t> {
    pub(crate) issue: usize,
    pub(crate) owner: usize,
    pub(crate) port: &'t str,
}

/// Lists the Entities of the tree below `root`, depth first in port order, at most
/// `max_parts` of them and at most [`MAX_ASSEMBLY_DEPTH`] levels deep.
pub(crate) fn flatten(root: &EntityNode, max_parts: usize) -> Flat<'_> {
    let mut walker = Walker {
        flat: Flat {
            drafts: vec![Draft {
                name: root.class_name().to_owned(),
                path: String::new(),
                port: None,
                gear: None,
                node: root,
                parent: None,
                geometry: root.geometry(),
                hidden: false,
            }],
            issues: Vec::new(),
            unresolved: Vec::new(),
        },
        full: false,
        max_parts,
    };
    walker.gears(root, 0, "");
    walker.ports(root.ports(), 0, "", 1);
    walker.flat
}

struct Walker<'t> {
    flat: Flat<'t>,
    full: bool,
    max_parts: usize,
}

impl<'t> Walker<'t> {
    fn ports(&mut self, ports: &'t [PortNode], owner: usize, prefix: &str, depth: usize) {
        for port in ports {
            if self.full {
                return;
            }
            let path = if prefix.is_empty() {
                port.name().to_owned()
            } else {
                format!("{prefix}/{}", port.name())
            };
            if depth > MAX_ASSEMBLY_DEPTH {
                self.issue(
                    &path,
                    port.name(),
                    IssueKind::TooDeep {
                        limit: MAX_ASSEMBLY_DEPTH,
                    },
                );
                continue;
            }
            match port.attachment() {
                Attachment::Entity(child) => self.attach(port, child, owner, &path, depth),
                Attachment::Unresolved(unresolved) => {
                    let name = match unresolved.target() {
                        Target::Reference(guid) => guid.to_string(),
                        Target::ClassName(name) => name.clone(),
                        Target::Unreadable => String::new(),
                    };
                    self.flat.unresolved.push(UnresolvedPort {
                        issue: self.flat.issues.len(),
                        owner,
                        port: port.name(),
                    });
                    self.issue(
                        &path,
                        &name,
                        IssueKind::Unresolved(unresolved.error().clone()),
                    );
                }
                Attachment::Empty => {}
            }
            self.ports(port.nested(), owner, &path, depth + 1);
        }
    }

    fn attach(
        &mut self,
        port: &'t PortNode,
        child: &'t EntityNode,
        owner: usize,
        path: &str,
        depth: usize,
    ) {
        if self.flat.drafts.len() >= self.max_parts {
            self.full = true;
            self.issue(
                path,
                child.class_name(),
                IssueKind::TooManyParts {
                    limit: self.max_parts,
                },
            );
            return;
        }
        let index = self.flat.drafts.len();
        let hidden = port.visibility() == PortVisibility::Invisible
            || self
                .flat
                .drafts
                .get(owner)
                .is_some_and(|draft| draft.hidden);
        self.flat.drafts.push(Draft {
            name: format!("{}/{}", port.name(), child.class_name()),
            path: path.to_owned(),
            port: Some(port),
            gear: None,
            node: child,
            parent: Some(owner),
            geometry: child.geometry(),
            hidden,
        });
        self.gears(child, index, path);
        self.ports(child.ports(), index, path, depth + 1);
    }

    fn gears(&mut self, node: &'t EntityNode, index: usize, path: &str) {
        if node.is_docked_vehicle() && !node.is_expanded() {
            return;
        }
        let hidden = self
            .flat
            .drafts
            .get(index)
            .is_some_and(|draft| draft.hidden);
        for gear in node.landing_gear() {
            if self.flat.drafts.len() >= self.max_parts {
                self.full = true;
                self.issue(
                    path,
                    gear.bone(),
                    IssueKind::TooManyParts {
                        limit: self.max_parts,
                    },
                );
                return;
            }
            let gear_path = if path.is_empty() {
                format!("landing_gear/{}", gear.bone())
            } else {
                format!("{path}/landing_gear/{}", gear.bone())
            };
            self.flat.drafts.push(Draft {
                name: format!("landing_gear/{}", gear.bone()),
                path: gear_path,
                port: None,
                gear: Some(gear),
                node,
                parent: Some(index),
                geometry: None,
                hidden,
            });
        }
    }

    fn issue(&mut self, path: &str, name: &str, kind: IssueKind) {
        self.flat.issues.push(Issue {
            path: path.to_owned(),
            name: name.to_owned(),
            kind,
        });
    }
}
