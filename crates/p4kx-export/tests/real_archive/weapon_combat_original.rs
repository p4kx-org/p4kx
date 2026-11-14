// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use p4kx_export::Kind;
use p4kx_extraction::{DataForge, Filter, Record, RecordTree, StructValue, TreeBuilder, Value};

use crate::real_archive_item_components;

pub(crate) const WEAPON: &str = "SCItemWeaponComponentParams";
const CONTAINER: &str = "SAmmoContainerComponentParams";
pub(super) const CHANNELS: &[(&str, &str)] = &[
    ("DamagePhysical", "physical"),
    ("DamageEnergy", "energy"),
    ("DamageDistortion", "distortion"),
    ("DamageThermal", "thermal"),
    ("DamageBiochemical", "biochemical"),
    ("DamageStun", "stun"),
];

pub(super) struct Witness<'source> {
    pub(super) owner: RecordTree<'source>,
    pub(super) ammo: RecordTree<'source>,
    pub(super) filter: Filter,
    pub(super) component: usize,
    pub(super) mode: usize,
    pub(super) source_type: &'source str,
    pub(super) magazine: Option<u32>,
}

pub(super) struct OriginalAction<'tree, 'source> {
    pub(super) action: &'tree Value<'source>,
    pub(super) rate: &'tree Value<'source>,
    pub(super) pellets: &'tree Value<'source>,
    pub(super) multiplier: &'tree Value<'source>,
    pub(super) role: &'tree Value<'source>,
}

pub(super) struct OriginalAmmo<'tree, 'source> {
    pub(super) projectile: &'tree Value<'source>,
    pub(super) damage: &'tree Value<'source>,
    pub(super) channels: &'tree StructValue<'source>,
    pub(super) speed: &'tree Value<'source>,
    pub(super) lifetime: &'tree Value<'source>,
}

pub(super) fn find<'source>(forge: &'source DataForge<'source>) -> Option<Witness<'source>> {
    let rule = Kind::Items.rule()?;
    let filter = Filter::new("/ships/weapons/");
    let probe = forge.pointer_probe("Components", WEAPON)?;
    let mut trees = TreeBuilder::new();
    let mut scopes = None;
    for owner in forge.records() {
        if !matches!(
            real_archive_item_components::selected_item(&owner, &rule, &filter),
            Ok(true)
        ) || !matches!(trees.holds(&probe, &owner), Ok(true))
        {
            continue;
        }
        let Ok(owner_tree) = trees.build(&owner) else {
            continue;
        };
        let scopes = scopes.get_or_insert_with(|| PathScopes::new(forge));
        if let Some(witness) = inspect(forge, owner_tree, scopes, &mut trees) {
            return Some(witness);
        }
    }
    None
}

fn inspect<'source>(
    forge: &'source DataForge<'source>,
    tree: RecordTree<'source>,
    scopes: &mut PathScopes<'source>,
    trees: &mut TreeBuilder<'source>,
) -> Option<Witness<'source>> {
    let (component, weapon) = unique(&tree, WEAPON)?;
    let magazine = magazine(forge, &tree, weapon, trees)?;
    let (ammo_owner, magazine_index) = match &magazine {
        MagazineSource::Inline => (&tree, None),
        MagazineSource::Linked(owner) => (owner.as_ref(), Some(owner.record_index())),
    };
    let (_, source) = unique(ammo_owner, CONTAINER)?;
    let source = structure(source)?;
    let raw = structure(weapon)?.field("fireActions")?;
    let actions = array(raw)?;
    if actions.iter().any(|value| matches!(value, Value::Error(_))) {
        return None;
    }
    for (mode, raw) in actions.iter().enumerate() {
        let Some(action) = action_inputs(raw) else {
            continue;
        };
        let Some(ammo) = selected_ammo(forge, source, action.role) else {
            continue;
        };
        let Ok(ammo_path) = ammo.record_path() else {
            continue;
        };
        let Some(filter) = scopes.narrow_filter(tree.record_path(), ammo_path) else {
            continue;
        };
        let Ok(ammo_tree) = trees.build(&ammo) else {
            continue;
        };
        if ammo_inputs(&ammo_tree).is_none() {
            continue;
        }
        let source_type = structure(raw)?.type_name();
        return Some(Witness {
            owner: tree,
            ammo: ammo_tree,
            filter,
            component,
            mode,
            source_type,
            magazine: magazine_index,
        });
    }
    None
}

pub(super) fn action_inputs<'tree, 'source>(
    raw: &'tree Value<'source>,
) -> Option<OriginalAction<'tree, 'source>> {
    let action = structure(raw)?;
    if !matches!(
        action.type_name(),
        "SWeaponActionFireSingleParams" | "SWeaponActionFireRapidParams"
    ) {
        return None;
    }
    let launch = structure(action.field("launchParams")?)?;
    if launch.type_name() != "SProjectileLauncher" {
        return None;
    }
    let rate = action.field("fireRate")?;
    let pellets = launch.field("pelletCount")?;
    let multiplier = launch.field("damageMultiplier")?;
    let role = launch.field("projectileType")?;
    if positive(rate) <= 0 || positive(pellets) <= 0 || single(multiplier).is_none() {
        return None;
    }
    Some(OriginalAction {
        action: raw,
        rate,
        pellets,
        multiplier,
        role,
    })
}

pub(super) fn ammo_inputs<'tree, 'source>(
    tree: &'tree RecordTree<'source>,
) -> Option<OriginalAmmo<'tree, 'source>> {
    let projectile = tree.field("projectileParams")?;
    let body = structure(projectile)?;
    if body.type_name() != "BulletProjectileParams" {
        return None;
    }
    let damage = body.field("damage")?;
    let channels = structure(damage)?;
    if channels.type_name() != "DamageInfo"
        || !CHANNELS.iter().any(|(name, _)| {
            channels
                .field(name)
                .and_then(single)
                .is_some_and(|value| value > 0.0)
        })
    {
        return None;
    }
    let speed = tree.field("speed")?;
    let lifetime = tree.field("lifetime")?;
    single(speed)?;
    single(lifetime)?;
    Some(OriginalAmmo {
        projectile,
        damage,
        channels,
        speed,
        lifetime,
    })
}

enum MagazineSource<'source> {
    Inline,
    Linked(Box<RecordTree<'source>>),
}

fn magazine<'source>(
    forge: &'source DataForge<'source>,
    owner: &RecordTree<'source>,
    weapon: &Value<'source>,
    trees: &mut TreeBuilder<'source>,
) -> Option<MagazineSource<'source>> {
    let weapon = structure(weapon)?;
    if !matches!(
        weapon.field("ShouldIgnorePrimaryAmmoContainer"),
        Some(Value::Bool(false))
    ) || !match weapon.field("secondaryAmmoContainers") {
        None | Some(Value::Null) => true,
        Some(Value::Array(values)) => values.is_empty(),
        _ => false,
    } {
        return None;
    }
    let magazine = weapon.field("ammoContainerRecord");
    if matches!(magazine, None | Some(Value::Null)) {
        return Some(MagazineSource::Inline);
    }
    let Some(Value::Reference(target)) = magazine else {
        return None;
    };
    if unique(owner, CONTAINER).is_some()
        || target.location().is_some()
        || target.type_name() != "EntityClassDefinition"
        || target.record_index() == owner.record_index()
    {
        return None;
    }
    trees
        .build(&forge.record(target.record_index())?)
        .ok()
        .map(Box::new)
        .map(MagazineSource::Linked)
}

fn selected_ammo<'source>(
    forge: &'source DataForge<'source>,
    owner: &StructValue<'source>,
    role: &Value<'source>,
) -> Option<Record<'source>> {
    let name = match role {
        Value::Enum("Primary") => "ammoParamsRecord",
        Value::Enum("Secondary") => "secondaryAmmoParamsRecord",
        _ => return None,
    };
    let Value::Reference(reference) = owner.field(name)? else {
        return None;
    };
    if reference.location().is_some() || reference.type_name() != "AmmoParams" {
        return None;
    }
    forge.record(reference.record_index())
}

pub(crate) fn unique<'tree, 'source>(
    tree: &'tree RecordTree<'source>,
    kind: &str,
) -> Option<(usize, &'tree Value<'source>)> {
    let values = array(tree.field("Components")?)?;
    if values.iter().any(|value| matches!(value, Value::Error(_))) {
        return None;
    }
    let mut matches = values
        .iter()
        .enumerate()
        .filter(|(_, value)| structure(value).is_some_and(|node| node.type_name() == kind));
    let current = matches.next()?;
    if matches.next().is_some() {
        return None;
    }
    Some(current)
}

pub(crate) const fn array<'tree, 'source>(
    value: &'tree Value<'source>,
) -> Option<&'tree [Value<'source>]> {
    if let Value::Array(values) = value {
        Some(values)
    } else {
        None
    }
}

pub(crate) const fn structure<'tree, 'source>(
    value: &'tree Value<'source>,
) -> Option<&'tree StructValue<'source>> {
    if let Value::Struct(body) = value {
        Some(body)
    } else {
        None
    }
}

pub(super) const fn positive(value: &Value<'_>) -> i32 {
    if let Value::Int32(number) = value {
        *number
    } else {
        0
    }
}

pub(super) fn single(value: &Value<'_>) -> Option<f64> {
    if let Value::Single(number) = value
        && number.is_finite()
        && *number >= 0.0
    {
        return Some(f64::from(*number));
    }
    None
}

struct PathScopes<'source> {
    paths: Vec<&'source str>,
    bounded: BTreeMap<String, bool>,
}

impl<'source> PathScopes<'source> {
    fn new(forge: &'source DataForge<'source>) -> Self {
        Self {
            paths: forge
                .records()
                .filter_map(|record| record.record_path().ok())
                .collect(),
            bounded: BTreeMap::new(),
        }
    }

    fn narrow_filter(&mut self, owner_path: &str, ammo_path: &str) -> Option<Filter> {
        const MAX_SCOPE: usize = 24;
        let owner = owner_path.rsplit(['/', '\\']).next()?;
        let ammo = ammo_path.rsplit(['/', '\\']).next()?.to_ascii_lowercase();
        for token in common_tokens(owner, &ammo) {
            let token = token.to_ascii_lowercase();
            let filter = Filter::new(&token);
            if !filter.matches(owner_path) || !filter.matches(ammo_path) {
                continue;
            }
            let bounded = self.bounded.entry(token).or_insert_with(|| {
                let matched = self
                    .paths
                    .iter()
                    .filter(|path| filter.matches(path))
                    .take(MAX_SCOPE + 1)
                    .count();
                (2..=MAX_SCOPE).contains(&matched)
            });
            if *bounded {
                return Some(filter);
            }
        }
        None
    }
}

fn common_tokens<'a>(owner: &'a str, ammo: &str) -> Vec<&'a str> {
    const MIN_TOKEN: usize = 7;
    let owner_ascii = owner.to_ascii_lowercase();
    if !owner.is_ascii() {
        return Vec::new();
    }
    let mut tokens = Vec::new();
    for start in 0..owner.len() {
        for end in (start.saturating_add(MIN_TOKEN)..=owner.len()).rev() {
            if owner_ascii
                .get(start..end)
                .is_some_and(|token| !token.contains(['*', '?']) && ammo.contains(token))
            {
                if let Some(token) = owner.get(start..end) {
                    tokens.push(token);
                }
                break;
            }
        }
    }
    tokens.sort_by_key(|token| std::cmp::Reverse(token.len()));
    tokens.dedup();
    tokens
}
