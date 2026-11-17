// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashSet, VecDeque};

use crate::shop_inventory_name::stem as inventory_stem;
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
const MAX_BACK: usize = 12;
const MAX_PLAUSIBLE_PROPERTY_NAME_LEN: usize = 256;

#[path = "shop_soc_anchors.rs"]
mod anchors;
use anchors::Anchor;
pub(crate) use anchors::Anchors;

/// One shop component found in a container's `.soc`: the Entity it belongs to and the inventory
/// file its parameters name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ShopComponent<Inventory = String> {
    pub(crate) entity: String,
    pub(crate) inventory: Option<Inventory>,
}

/// What reading one container file found.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Scan<Inventory = String> {
    pub(crate) components: Vec<ShopComponent<Inventory>>,
    /// The inventory paths no anchor could be tied to.
    pub(crate) rejected: usize,
    /// A populated inventory value could not be represented.
    pub(crate) malformed: bool,
    #[cfg(test)]
    pub(crate) raw_findings: Vec<RawFinding>,
}

#[cfg(test)]
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct RawFinding {
    pub(crate) value_offset: usize,
    pub(crate) invalid: Option<RawInvalid>,
}

#[cfg(test)]
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct RawInvalid {
    pub(crate) property_offset: usize,
    pub(crate) owner: Option<String>,
}

/// Projects the shared shop-component scanner without source-cell materialization.
#[cfg(test)]
pub(crate) fn components(bytes: &[u8], anchors: &Anchors) -> Scan {
    scan(
        bytes,
        anchors,
        |_, _, stem, _| Ok::<_, std::convert::Infallible>(stem),
        |_, _, _, _| Ok(()),
    )
    .unwrap_or_else(|never| match never {})
}

#[path = "shop_soc_observation.rs"]
mod observation;
#[cfg(test)]
pub(crate) use observation::CapturedScan;
pub(crate) use observation::read_components;

impl<Inventory> Default for Scan<Inventory> {
    fn default() -> Self {
        Self {
            components: Vec::new(),
            rejected: 0,
            malformed: false,
            #[cfg(test)]
            raw_findings: Vec::new(),
        }
    }
}

#[derive(Clone, Copy)]
struct Token<'bytes> {
    text: &'bytes str,
    offset: usize,
}
struct Owner<'anchors> {
    anchor: Anchor<'anchors>,
    start: usize,
}

pub(super) enum Notice {
    Marker,
    Rejected,
}

fn scan<'bytes, 'anchors, Inventory, Error>(
    bytes: &'bytes [u8],
    anchors: &'anchors Anchors,
    mut capture: impl FnMut(
        Token<'bytes>,
        &VecDeque<Token<'bytes>>,
        String,
        Owner<'anchors>,
    ) -> Result<Inventory, Error>,
    mut observe: impl FnMut(
        Notice,
        Token<'bytes>,
        &VecDeque<Token<'bytes>>,
        Option<&Owner<'anchors>>,
    ) -> Result<(), Error>,
) -> Result<Scan<Inventory>, Error> {
    if !contains(bytes, PROPERTY.as_bytes()) && !contains(bytes, MARK.as_bytes()) {
        return Ok(Scan::default());
    }
    let mut window: VecDeque<Token<'_>> = VecDeque::with_capacity(MAX_BACK);
    let mut state = State::new();
    let mut offset = 0;
    for (at, part) in bytes.split(|&byte| byte == 0).enumerate() {
        let text = std::str::from_utf8(part).unwrap_or_default();
        let token = Token { text, offset };
        if text == MARK {
            if let Some(owner) = owner(&window, anchors) {
                observe(Notice::Marker, token, &window, Some(&owner))?;
                state.mark(at, Some(owner.anchor.entity));
            }
        } else if let Some(stem) = inventory_stem(text)
            && (is_inventory_folder(text)
                || window.back().is_some_and(|last| is_property(last.text)))
        {
            if let Some(owner) = owner(&window, anchors) {
                let entity = owner.anchor.entity.to_owned();
                let value = capture(token, &window, stem, owner)?;
                state.path(value, entity);
            } else {
                state.rejected += 1;
                state.uncertain(at);
                #[cfg(test)]
                state.raw_findings.push(RawFinding {
                    value_offset: token.offset,
                    invalid: None,
                });
            }
        } else if window.back().is_some_and(|last| is_property(last.text))
            && invalid_inventory_value(part)
        {
            let selected = owner(&window, anchors);
            #[cfg(test)]
            if let Some(property) = window.back() {
                state.raw_findings.push(RawFinding {
                    value_offset: token.offset,
                    invalid: Some(RawInvalid {
                        property_offset: property.offset,
                        owner: selected
                            .as_ref()
                            .map(|owner| owner.anchor.entity.to_owned()),
                    }),
                });
            }
            observe(Notice::Rejected, token, &window, selected.as_ref())?;
            state.malformed = true;
            state.uncertain(at);
        }
        if window.len() == MAX_BACK {
            window.pop_front();
        }
        window.push_back(token);
        offset += part.len() + usize::from(offset + part.len() < bytes.len());
    }
    Ok(state.finish())
}

struct State<Inventory> {
    found: Vec<ShopComponent<Inventory>>,
    uncertain: HashSet<usize>,
    last_mark: Option<(usize, usize)>,
    rejected: usize,
    malformed: bool,
    #[cfg(test)]
    raw_findings: Vec<RawFinding>,
}

impl<Inventory> State<Inventory> {
    fn new() -> Self {
        Self {
            found: Vec::new(),
            uncertain: HashSet::new(),
            last_mark: None,
            rejected: 0,
            malformed: false,
            #[cfg(test)]
            raw_findings: Vec::new(),
        }
    }

    fn mark(&mut self, at: usize, owner: Option<&str>) {
        let Some(entity) = owner else {
            return;
        };
        let index = self
            .found
            .iter()
            .position(|component| component.entity == entity)
            .unwrap_or_else(|| {
                self.found.push(ShopComponent {
                    entity: entity.to_owned(),
                    inventory: None,
                });
                self.found.len() - 1
            });
        self.last_mark = Some((index, at));
    }

    fn path(&mut self, value: Inventory, entity: String) {
        match self
            .found
            .iter_mut()
            .find(|component| component.entity == entity && component.inventory.is_none())
        {
            Some(component) => component.inventory = Some(value),
            None => self.found.push(ShopComponent {
                entity,
                inventory: Some(value),
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

    fn finish(self) -> Scan<Inventory> {
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
            #[cfg(test)]
            raw_findings: self.raw_findings,
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
    text.len() > MAX_PLAUSIBLE_PROPERTY_NAME_LEN || text.contains(['/', '\\', '.', ' '])
}

fn owner<'anchors>(
    window: &VecDeque<Token<'_>>,
    anchors: &'anchors Anchors,
) -> Option<Owner<'anchors>> {
    let (mut positions, mut marks) = (0_usize, 0_usize);
    for at in (0..window.len()).rev() {
        let text = window.get(at)?.text;
        if let Some(entity) = anchors.entity(text) {
            return Some(Owner {
                anchor: entity,
                start: at,
            });
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
                .is_some_and(|before| before.text == "SCShop");
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

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

#[cfg(test)]
#[path = "shop_soc/tests.rs"]
mod tests;
