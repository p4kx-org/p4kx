// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_export::Kind;
use p4kx_export::catalog_queries::equipment::{WeaponAction, WeaponSourceSnapshot};
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordCoverage, CatalogRecordSelection,
};
use p4kx_export::mining_locations::ResourceFloat32;
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForge, DataForgeEntry, DataForgeOrigin, DataType, Filter,
    InputScope, RecordTree, SourceEncryption, SourceInput, SourceUses, StructValue, TreeBuilder,
    Value as Raw,
};
use p4kx_testkit::{real_archive_b_path, real_archive_key, real_archive_path};

use crate::real_archive_item_components::{cell, selected_item};
use crate::real_archive_item_durability::{self as proof, Trace};
use crate::real_archive_verification;
use crate::real_archive_weapon_combat::original::{self, WEAPON};

#[path = "weapon_recoil_sdk_definition.rs"]
mod definition;

use definition::Counts;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const DEFINITION: &str = "WeaponProceduralRecoilConfigDef";
const UNITS: &[(&str, &str, &str)] = &[
    (
        "weaponProceduralHandsRecoil",
        "hands",
        "SWeaponProceduralHandsRecoilConfigDef",
    ),
    (
        "weaponProceduralAimRecoil",
        "aim",
        "SWeaponProceduralAimRecoilConfigDef",
    ),
    (
        "weaponProceduralBodyRecoil",
        "body",
        "SWeaponProceduralBodyRecoilConfigDef",
    ),
    (
        "weaponProceduralHeadRecoil",
        "head",
        "SWeaponProceduralHeadRecoilConfigDef",
    ),
];

#[derive(Clone, Copy)]
enum Link {
    Recoil,
    Grappling,
}

impl Link {
    const fn name(self) -> &'static str {
        match self {
            Self::Recoil => "recoil",
            Self::Grappling => "recoilGrappling",
        }
    }
}

struct Witness<'source> {
    owner: RecordTree<'source>,
    target: RecordTree<'source>,
    component: usize,
    slot: usize,
    link: Link,
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn weapon_recoil_sdk_matches_original_action_and_definition() -> TestResult {
    let paths = std::iter::once(real_archive_path()?).chain(real_archive_b_path());
    let key = real_archive_key().map_err(|_| "configured Archive key")?;
    for (index, path) in paths.enumerate() {
        let archive =
            Archive::open_with_key(path, Some(key.clone())).map_err(|_| "open input Archive")?;
        let build = archive.build().ok_or("complete actual Build identity")?;
        if [
            build.branch(),
            build.version(),
            build.change_number(),
            build.build_id(),
        ]
        .into_iter()
        .any(|value| value.trim().is_empty())
        {
            return Err("the installed recoil proof requires a complete Build identity".into());
        }
        let counts = verify(&archive)?;
        real_archive_verification::print_line(&format!(
            "recoil SDK source witness {}: one action and one physical definition, {} present units, {} original arena nodes",
            index + 1,
            counts.units,
            counts.nodes
        ));
    }
    Ok(())
}

fn verify(archive: &Archive) -> TestResult<Counts> {
    let watch = ArchiveWatch::new(archive);
    let entry = DataForgeEntry::read(archive).map_err(|_| "read actual DataForge")?;
    let forge = entry.open().map_err(|_| "open actual DataForge")?;
    let witness = find(&forge).ok_or("readable authored weapon recoil action and definition")?;
    watch
        .check()
        .map_err(|_| "Archive changed during recoil source selection")?;
    let session = CatalogQuerySession::from_entry(
        archive,
        &entry,
        CatalogLanguageSelection::Selected(&[]),
        None,
    )
    .map_err(|_| "bind the decoded DataForge to the recoil SDK")?;
    let parsed = session
        .parsed(None)
        .map_err(|_| "open parsed recoil SDK session")?;
    parsed
        .with_weapon_sources(witness.owner.record_index(), None, |snapshot| {
            if snapshot.build() != archive.build() {
                return Err("the action SDK must preserve the exact input Build".into());
            }
            action(&entry, &witness, &snapshot)
        })
        .map_err(|_| "finish Archive-bound action SDK read")??;
    let target = CatalogRecordSelection::RecordIndex(witness.target.record_index());
    let counts = parsed
        .with_weapon_recoil_definitions(target, None, |snapshot| {
            if snapshot.build() != archive.build() {
                return Err("the recoil SDK must preserve the exact input Build".into());
            }
            definition::verify(&entry, &witness.target, &snapshot)
        })
        .map_err(|_| "finish Archive-bound recoil SDK read")??;
    watch
        .check()
        .map_err(|_| "Archive changed during recoil verification")?;
    Ok(counts)
}

fn find<'source>(forge: &'source DataForge<'source>) -> Option<Witness<'source>> {
    let rule = Kind::Items.rule()?;
    let paths = Filter::new("/weapons/");
    let probe = forge.pointer_probe("Components", WEAPON)?;
    let mut trees = TreeBuilder::new();
    for record in forge.records() {
        if !matches!(selected_item(&record, &rule, &paths), Ok(true))
            || !matches!(trees.holds(&probe, &record), Ok(true))
        {
            continue;
        }
        let Ok(owner) = trees.build(&record) else {
            continue;
        };
        if let Some(witness) = find_action(forge, &mut trees, owner) {
            return Some(witness);
        }
    }
    None
}

fn find_action<'source>(
    forge: &'source DataForge<'source>,
    trees: &mut TreeBuilder<'source>,
    owner: RecordTree<'source>,
) -> Option<Witness<'source>> {
    let (component, raw) = original::unique(&owner, WEAPON)?;
    let weapon = original::structure(raw)?;
    let actions = original::array(weapon.field("fireActions")?)?;
    for (slot, raw) in actions.iter().enumerate() {
        let Some(action) = original::structure(raw) else {
            continue;
        };
        if !matches!(action.field("recoilInterval"), Some(Raw::Single(_))) {
            continue;
        }
        for link in [Link::Recoil, Link::Grappling] {
            let Some(target) = reached_definition(forge, trees, action, link) else {
                continue;
            };
            return Some(Witness {
                owner,
                target,
                component,
                slot,
                link,
            });
        }
    }
    None
}

fn reached_definition<'source>(
    forge: &'source DataForge<'source>,
    trees: &mut TreeBuilder<'source>,
    action: &StructValue<'source>,
    link: Link,
) -> Option<RecordTree<'source>> {
    let caller = action
        .fields()
        .iter()
        .find(|field| field.name() == link.name())?;
    if caller.storage().data_type() != DataType::Reference || caller.storage().is_array() {
        return None;
    }
    let Raw::Reference(reference) = caller.value() else {
        return None;
    };
    if reference.type_name() != DEFINITION || reference.location().is_some() {
        return None;
    }
    let record = forge.record(reference.record_index())?;
    if !reference.original_target().shares_source_with(&record) {
        return None;
    }
    let target = trees.build(&record).ok()?;
    if target.guid() != reference.guid() || target.type_name() != DEFINITION {
        return None;
    }
    let positive_witness = UNITS.iter().any(|(raw, _, kind)| {
        target
            .field(raw)
            .and_then(original::structure)
            .is_some_and(|unit| {
                unit.type_name() == *kind && unit.fields().iter().any(|field| {
                matches!(field.value(), Raw::Single(number) if number.is_finite() && *number != 0.0)
            })
            })
    });
    positive_witness.then_some(target)
}

fn action(
    entry: &DataForgeEntry,
    witness: &Witness<'_>,
    snapshot: &WeaponSourceSnapshot<'_>,
) -> TestResult {
    check_coverage(
        snapshot.coverage(),
        snapshot.requested_languages().is_empty(),
    )?;
    let [owner] = snapshot.rows() else {
        return Err("one selected physical recoil weapon owner".into());
    };
    let [weapon] = owner.weapon_actions.as_slice() else {
        return Err("one original weapon component".into());
    };
    let component = u32::try_from(witness.component)?;
    if weapon.owner().record_index().value() != Some(&witness.owner.record_index())
        || weapon.owner().component_ordinal().value() != Some(&component)
    {
        return Err("SDK action row must match the physical component".into());
    }
    let (_, raw) =
        original::unique(&witness.owner, WEAPON).ok_or("one original weapon component")?;
    let raw_action = original::structure(raw)
        .and_then(|weapon| weapon.field("fireActions"))
        .and_then(original::array)
        .and_then(|actions| actions.get(witness.slot))
        .ok_or("original recoil action at the selected slot")?;
    let node = weapon
        .fire_actions()
        .value()
        .and_then(|slots| slots.get(witness.slot))
        .and_then(|slot| slot.value())
        .ok_or("SDK original fire slot")?;
    let action = weapon
        .action_nodes()
        .get(usize::try_from(*node)?)
        .ok_or("SDK original action node")?;
    let inventory = serde_json::to_value(snapshot.source_inventory())?;
    let uses = proof::indexed(&inventory, "/uses", "id")?;
    let (scope, encrypted) = source_scope(entry)?;
    let trace = Trace::of(&witness.owner, &uses, &scope, encrypted);
    if action.source_ordinal().value().map(|value| value.get())
        != Some(u64::try_from(trace.ordinal(raw_action)?)?)
    {
        return Err("SDK recoil link must come from the exact original action occurrence".into());
    }
    check_action_values(&trace, raw_action, action, witness)
}

fn check_action_values<'source>(
    trace: &Trace<'_, 'source>,
    raw: &Raw<'source>,
    actual: &WeaponAction<'_>,
    witness: &Witness<'_>,
) -> TestResult {
    let raw = original::structure(raw).ok_or("readable original action")?;
    let link = witness.link;
    let raw_link = raw.field(link.name()).ok_or("original recoil selector")?;
    let projected = match link {
        Link::Recoil => actual.recoil().target(),
        Link::Grappling => actual.recoil().grappling_target(),
    };
    let target = projected
        .value()
        .ok_or("one typed original recoil target")?;
    if target.resolved_index() != Some(witness.target.record_index())
        || target.guid().value() != Some(&witness.target.guid())
    {
        return Err(
            "the SDK recoil target must match the independently selected physical Record".into(),
        );
    }
    let interval = raw
        .field("recoilInterval")
        .ok_or("original recoil interval")?;
    let Raw::Single(expected) = interval else {
        return Err("the original recoil interval has Single storage".into());
    };
    if actual
        .recoil()
        .interval()
        .value()
        .map(ResourceFloat32::raw_bits)
        != Some(expected.to_bits())
    {
        return Err("the SDK interval must preserve the original Single bits".into());
    }
    let serialized = serde_json::to_value(actual)?;
    trace.check(cell(&serialized, link.name())?, raw_link, "selector")?;
    trace.check(cell(&serialized, "recoilInterval")?, interval, "payload")
}

fn check_coverage(coverage: &CatalogRecordCoverage, no_requested_languages: bool) -> TestResult {
    if !coverage.unreadable_headers.is_empty()
        || !coverage.unreadable_bodies.is_empty()
        || !coverage.incomplete_fields.is_empty()
        || !no_requested_languages
    {
        return Err("the selected recoil source must be readable without localization".into());
    }
    Ok(())
}

fn source_scope(
    entry: &DataForgeEntry,
) -> TestResult<(p4kx_extraction::SourceScope, Option<bool>)> {
    let DataForgeOrigin::Archive(origin) = entry.origin() else {
        return Err("the recoil DataForge must have Archive origin".into());
    };
    let encrypted = match entry.origin().encryption() {
        SourceEncryption::Encrypted => Some(true),
        SourceEncryption::Unencrypted => Some(false),
        _ => None,
    };
    let uses = SourceUses::new();
    let scope = uses.observe(SourceInput::Archive(origin), InputScope::Payload)?;
    Ok((scope, encrypted))
}
