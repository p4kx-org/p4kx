// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_cryxml::SourceAttributePosition;
use p4kx_extraction::SourceLocation;

use crate::loadout_xml::{AttributeNamespaces, Node, Scope};
use crate::markup::{self, Budget, Element};

/// The most bytes of a skeleton's parameter file that are read.
pub(crate) const MAX_CHRPARAMS_BYTES: u64 = 1 << 20;

/// The most folders and databases of a parameter file that are kept.
pub(crate) const MAX_PARAM_SOURCES: usize = 64;

const NAMESPACES: AttributeNamespaces = AttributeNamespaces {
    text_bytes: "gearParameterTextBytes",
    cryxml_attribute: "gearParameterCryXmlAttribute",
};

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct ParameterSource {
    name: SourceAttributePosition,
    path: SourceAttributePosition,
}

impl ParameterSource {
    fn read(node: Node<'_, '_>, scope: &Scope<'_, '_>) -> Option<Self> {
        Some(Self {
            name: scope.attribute_ascii_position(node, "name")?,
            path: scope.attribute_ascii_position(node, "path")?,
        })
    }

    pub(crate) fn locations(
        &self,
        fields: [&str; 2],
    ) -> impl Iterator<Item = SourceLocation> + use<> {
        [
            NAMESPACES.location(&self.name, fields[0]),
            NAMESPACES.location(&self.path, fields[1]),
        ]
        .into_iter()
        .flatten()
    }
}

#[derive(Debug, PartialEq, Eq)]
struct DeployClip {
    priority: DeployPriority,
    path: String,
    source: Option<ParameterSource>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum DeployPriority {
    Deploy,
    Extend,
}

impl DeployPriority {
    fn of(name: &str) -> Option<Self> {
        let name = name.to_ascii_lowercase();
        if name.contains("retract") || name.contains("compress") {
            return None;
        }
        if name.contains("extend") {
            Some(Self::Extend)
        } else if name.contains("deploy") {
            Some(Self::Deploy)
        } else {
            None
        }
    }
}

/// Animation folders, databases and the effective deploy clip from one skeleton parameter file.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct ChrParams {
    pub(crate) folders: Vec<String>,
    pub(crate) databases: Vec<String>,
    pub(crate) capped: bool,
    pub(crate) folder_source: Option<ParameterSource>,
    pub(crate) database_source: Option<ParameterSource>,
    deploy: Option<DeployClip>,
}

impl ChrParams {
    /// Returns the last authored folder and database, used for landing gear.
    pub(crate) fn last_sources(&self) -> (Option<&str>, Option<&str>) {
        (
            self.folders.last().map(String::as_str),
            self.databases.last().map(String::as_str),
        )
    }

    /// Returns the effective deploy clip.
    pub(crate) fn deploy_clip(&self) -> Option<&str> {
        self.deploy.as_ref().map(|clip| clip.path.as_str())
    }

    pub(crate) fn deploy_source(&self) -> Option<&ParameterSource> {
        self.deploy.as_ref()?.source.as_ref()
    }

    fn choose_deploy(
        &mut self,
        name: &str,
        path: &str,
        source: impl FnOnce() -> Option<ParameterSource>,
    ) {
        if let Some(priority) = DeployPriority::of(name)
            && self
                .deploy
                .as_ref()
                .is_none_or(|current| priority > current.priority)
        {
            self.deploy = Some(DeployClip {
                priority,
                path: path.to_owned(),
                source: source(),
            });
        }
    }
}

pub(crate) fn read_params(bytes: &[u8]) -> Result<ChrParams, String> {
    let mut budget = Budget::new(bytes.len());
    let root = markup::parse(bytes, &mut budget)?;
    project(&root, |_, _| None)
}

pub(crate) fn read_params_with_positions(bytes: &[u8]) -> Result<ChrParams, String> {
    let mut budget = Budget::new(bytes.len());
    markup::parse_with_source(bytes, &mut budget, |root, original, scope| {
        let lists = positioned_lists(original);
        project(&root, |list, entry| {
            ParameterSource::read(*lists.get(list)?.get(entry)?, scope)
        })
    })
}

/// One complete authored animation parameter without path expansion or deduplication.
#[derive(Debug)]
#[non_exhaustive]
pub struct RigParameter {
    /// Original `AnimationList` child index in the parameter document.
    pub list: usize,
    /// Original child index in that list.
    pub ordinal: usize,
    /// Original name, including folder, database and wildcard directives.
    pub name: String,
    /// Original path or pattern.
    pub path: String,
    /// Positions of the name and path in the parameter source.
    pub selectors: Vec<SourceLocation>,
}

pub(crate) fn catalogue_parameters(bytes: &[u8]) -> Result<Vec<RigParameter>, String> {
    crate::loadout_xml::parse_unbound(bytes, |root, scope| {
        check_root(root.name())?;
        let mut result = Vec::new();
        for (list, node) in root.children().into_iter().enumerate() {
            if !node.name().eq_ignore_ascii_case("AnimationList") {
                continue;
            }
            for (ordinal, item) in node.children().into_iter().enumerate() {
                if !item.name().eq_ignore_ascii_case("Animation") {
                    continue;
                }
                let (Some((_, name)), Some((_, path))) =
                    (item.attribute_ascii("name"), item.attribute_ascii("path"))
                else {
                    continue;
                };
                result.push(RigParameter {
                    list,
                    ordinal,
                    name: name.to_owned(),
                    path: path.to_owned(),
                    selectors: ParameterSource::read(item, scope)
                        .into_iter()
                        .flat_map(|source| source.locations(["name", "path"]))
                        .collect(),
                });
            }
        }
        Ok(result)
    })
}

fn positioned_lists<'node, 'doc>(original: Node<'node, 'doc>) -> Vec<Vec<Node<'node, 'doc>>> {
    original
        .children()
        .into_iter()
        .map(|list| {
            if list.name().eq_ignore_ascii_case("AnimationList") {
                list.children()
            } else {
                Vec::new()
            }
        })
        .collect()
}

fn project(
    root: &Element,
    mut source: impl FnMut(usize, usize) -> Option<ParameterSource>,
) -> Result<ChrParams, String> {
    let mut params = ChrParams::default();
    visit_parameters(root, |list, entry, name, file| match name {
        "#filepath" => {
            params.capped |= keep_last(&mut params.folders, file.replace('\\', "/"));
            params.folder_source = source(list, entry);
        }
        "$TracksDatabase" => {
            params.capped |= keep_last(&mut params.databases, file.replace('\\', "/"));
            params.database_source = source(list, entry);
        }
        _ => params.choose_deploy(name, file, || source(list, entry)),
    })?;
    Ok(params)
}

fn visit_parameters(
    root: &Element,
    mut visit: impl FnMut(usize, usize, &str, &str),
) -> Result<(), String> {
    check_root(&root.name)?;
    for (list, items) in root.children.iter().enumerate() {
        if !items.name.eq_ignore_ascii_case("AnimationList") {
            continue;
        }
        for (entry, animation) in items.children.iter().enumerate() {
            if animation.name.eq_ignore_ascii_case("Animation")
                && let (Some(name), Some(file)) =
                    (animation.attribute("name"), animation.attribute("path"))
            {
                visit(list, entry, name, file);
            }
        }
    }
    Ok(())
}

fn check_root(name: &str) -> Result<(), String> {
    if name.eq_ignore_ascii_case("Params") {
        Ok(())
    } else {
        Err(format!("the root element is {name}, not Params"))
    }
}

fn keep_last(list: &mut Vec<String>, path: String) -> bool {
    if list.len() < MAX_PARAM_SOURCES {
        list.push(path);
        false
    } else {
        if let Some(last) = list.last_mut() {
            *last = path;
        }
        true
    }
}
