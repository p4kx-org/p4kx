// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, HashSet, VecDeque};

use crate::shop_registry::is_guid;

const MARK: &str = "EntityComponentShop";
const PROPERTY: &str = "offlineInventory";
const KNOWN: [&str; 10] = [
    "SCShop",
    MARK,
    "SShopComponentParams",
    "PropertiesDataCore",
    "__fullyInstanced",
    "__version",
    "__type",
    "CastShadowMinSpec",
    "AllowBakedRendering",
    "RenderProxy",
];
const MAX_PATH_LEN: usize = 256;
const MAX_BACK: usize = 12;

/// The identifiers a shop component can be tied to inside one container: the `CryGUID`s of its
/// `entitycomponentshop.xml` and the names of its `SCShop` objects, both of which the container
/// states for the same Entities.
#[derive(Debug, Default)]
pub(crate) struct Anchors {
    guids: HashSet<String>,
    names: HashMap<String, String>,
}

impl Anchors {
    /// Anchors for the lower-case `guids`, with the `names` of the objects that are unique in the
    /// container, each mapped to its lower-case `CryGUID`.
    pub(crate) fn new(guids: &[String], names: HashMap<String, String>) -> Self {
        let guids: HashSet<String> = guids.iter().cloned().collect();
        let names = names
            .into_iter()
            .filter(|(_, guid)| guids.contains(guid))
            .collect();
        Self { guids, names }
    }

    fn entity(&self, text: &str) -> Option<String> {
        if let Some(guid) = self.names.get(text) {
            return Some(guid.clone());
        }
        if is_guid(text) {
            let lower = text.to_ascii_lowercase();
            return self.guids.contains(&lower).then_some(lower);
        }
        None
    }
}

/// One shop component found in a container's `.soc`: the Entity it belongs to and the inventory
/// file its parameters name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ShopComponent {
    pub(crate) entity: String,
    pub(crate) inventory: Option<String>,
}

/// What reading one container file found.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Scan {
    pub(crate) components: Vec<ShopComponent>,
    /// The inventory paths no anchor could be tied to.
    pub(crate) rejected: usize,
    /// A populated inventory value could not be represented.
    pub(crate) malformed: bool,
}

/// Returns the shop components of the container file `bytes` that belong to an Entity of
/// `anchors`.
///
/// The file stores its strings once, in order of first use, so a later component repeats neither
/// the component name nor the property names. The inventory path is the value that follows an
/// `offlineInventory*` property name, or, where that name is not repeated, a path in a
/// `ShopInventories` folder; it belongs to the Entity whose `CryGUID` or object name is the
/// closest anchor before it, provided only the component name (once), the parameter type,
/// property names, class names, one position and, directly after `SCShop`, the class GUID stand
/// between them. A path with anything else in between is counted in `rejected` and its Entity
/// stays unknown. A component name before which such an anchor stands and for which no path was
/// found names no inventory, unless a path close behind it was rejected: then nothing is known.
pub(crate) fn components(bytes: &[u8], anchors: &Anchors) -> Scan {
    if !contains(bytes, PROPERTY.as_bytes()) && !contains(bytes, MARK.as_bytes()) {
        return Scan::default();
    }
    let mut window: VecDeque<&str> = VecDeque::with_capacity(MAX_BACK);
    let mut state = State::default();
    for (at, part) in bytes.split(|&byte| byte == 0).enumerate() {
        let text = std::str::from_utf8(part).unwrap_or_default();
        if text == MARK {
            state.mark(at, owner(&window, anchors));
        } else if let Some(stem) = inventory_stem(text)
            && (is_inventory_folder(text) || window.back().is_some_and(|last| is_property(last)))
        {
            state.path(at, stem, owner(&window, anchors));
        } else if window.back().is_some_and(|last| is_property(last))
            && invalid_inventory_value(part)
        {
            state.malformed = true;
            state.uncertain(at);
        }
        if window.len() == MAX_BACK {
            window.pop_front();
        }
        window.push_back(text);
    }
    state.finish()
}

#[derive(Default)]
struct State {
    found: Vec<ShopComponent>,
    uncertain: HashSet<usize>,
    last_mark: Option<(usize, usize)>,
    rejected: usize,
    malformed: bool,
}

impl State {
    fn mark(&mut self, at: usize, owner: Option<String>) {
        let Some(entity) = owner else {
            return;
        };
        let index = self
            .found
            .iter()
            .position(|component| component.entity == entity)
            .unwrap_or_else(|| {
                self.found.push(ShopComponent {
                    entity,
                    inventory: None,
                });
                self.found.len() - 1
            });
        self.last_mark = Some((index, at));
    }

    fn path(&mut self, at: usize, stem: String, owner: Option<String>) {
        let Some(entity) = owner else {
            self.rejected += 1;
            self.uncertain(at);
            return;
        };
        match self
            .found
            .iter_mut()
            .find(|component| component.entity == entity && component.inventory.is_none())
        {
            Some(component) => component.inventory = Some(stem),
            None => self.found.push(ShopComponent {
                entity,
                inventory: Some(stem),
            }),
        }
    }

    fn uncertain(&mut self, at: usize) {
        if let Some((index, mark)) = self.last_mark
            && at - mark <= MAX_BACK
        {
            self.uncertain.insert(index);
        }
    }

    fn finish(self) -> Scan {
        let components = self
            .found
            .into_iter()
            .enumerate()
            .filter(|(index, component)| {
                component.inventory.is_some() || !self.uncertain.contains(index)
            })
            .map(|(_, component)| component)
            .collect();
        Scan {
            components,
            rejected: self.rejected,
            malformed: self.malformed,
        }
    }
}

fn invalid_inventory_value(part: &[u8]) -> bool {
    if part.is_empty() {
        return false;
    }
    let Ok(text) = std::str::from_utf8(part) else {
        return true;
    };
    // An omitted value can place the next property name directly after this property.
    text.len() > MAX_PATH_LEN || text.contains(['/', '\\', '.', ' '])
}

fn owner(window: &VecDeque<&str>, anchors: &Anchors) -> Option<String> {
    let (mut positions, mut marks) = (0_usize, 0_usize);
    for at in (0..window.len()).rev() {
        let text = *window.get(at)?;
        if let Some(entity) = anchors.entity(text) {
            return Some(entity);
        }
        if text == MARK {
            marks += 1;
            if marks > 1 {
                return None;
            }
        } else if is_guid(text) {
            let class = at
                .checked_sub(1)
                .and_then(|before| window.get(before))
                .is_some_and(|before| *before == "SCShop");
            if !class {
                return None;
            }
        } else if is_position(text) {
            positions += 1;
            if positions > 1 {
                return None;
            }
        } else if !(text.starts_with(PROPERTY) || KNOWN.contains(&text)) {
            return None;
        }
    }
    None
}

fn is_property(text: &str) -> bool {
    text.strip_prefix(PROPERTY)
        .is_some_and(|rest| rest.bytes().all(|byte| byte.is_ascii_alphabetic()))
}

fn is_inventory_folder(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.contains("shopinventories/") || lower.contains("shopinventories\\")
}

fn is_position(text: &str) -> bool {
    text.bytes().any(|byte| byte.is_ascii_digit())
        && text.bytes().any(|byte| matches!(byte, b'.' | b','))
        && text
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'-' | b'+' | b'.' | b',' | b'e'))
}

/// Returns the lower-case file stem of an inventory file `value`, a path ending in `.json` or
/// `.xml`; `None` for anything else.
pub(crate) fn inventory_stem(value: &str) -> Option<String> {
    if value.len() > MAX_PATH_LEN
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_graphic() || byte == b' ')
    {
        return None;
    }
    let file = value.rsplit(['/', '\\']).next()?;
    let (stem, extension) = file.rsplit_once('.')?;
    let known = extension.eq_ignore_ascii_case("json") || extension.eq_ignore_ascii_case("xml");
    (known && !stem.is_empty()).then(|| stem.to_ascii_lowercase())
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

#[cfg(test)]
mod tests;
