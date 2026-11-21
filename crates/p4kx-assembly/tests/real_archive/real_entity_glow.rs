// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::ops::Range;

use p4kx_assembly::{
    ArchiveFiles, ArchiveModels, Assembly, AssemblyOptions, EntityGlowLink, GlowEffectKind,
    GlowSelection, GlowSubset, LoadoutResolver, Part,
};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForgeEntry, DataForgeOrigin, FieldPath, SourceEncryption,
    SourceLocation, StructValue, Value,
};
use p4kx_model::{Model, ModelNodeSources};
use p4kx_testkit::{real_archive_key, real_archive_path};

#[path = "real_entity_glow_model.rs"]
mod original_model;

use original_model::PrimaryModel;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and P4KX_PRIVATE_GLOW_SELECTOR"]
fn installed_authored_glow_joins_the_original_owner_nodes_and_material_slots() -> TestResult {
    let path = real_archive_path().map_err(|_| "configure the installed Archive")?;
    let key = real_archive_key().map_err(|_| "read the configured Archive key")?;
    let archive = Archive::open_with_key(path, Some(key)).map_err(|_| "open the Archive")?;
    let watch = ArchiveWatch::new(&archive);
    let selector = std::env::var_os("P4KX_PRIVATE_GLOW_SELECTOR")
        .ok_or("configure the private DataForge Record path file")?;
    let selector = std::fs::read_to_string(selector).map_err(|_| "read the private selector")?;
    verify(&archive, selector.trim())?;
    watch.check().map_err(|_| "installed Archive changed")?;
    Ok(())
}

fn verify(archive: &Archive, selector: &str) -> TestResult {
    let input = DataForgeEntry::read(archive).map_err(|_| "read installed DataForge")?;
    let forge = input.open()?;
    let mut records = forge.records_at_path(selector);
    let record = records.next().ok_or("original Entity Record absent")?;
    check(
        records.next().is_none(),
        "ambiguous private Entity selector",
    )?;
    let original = record.tree().map_err(|_| "read original Glow Record")?;
    let expected = authored_links(original.root());
    check(!expected.is_empty(), "no original Glow link")?;
    let files = ArchiveFiles::new(archive);
    let mut resolver = LoadoutResolver::new(&forge)
        .with_forge_entry(&input)
        .with_files(&files);
    let loadout = resolver
        .resolve_tree(&record, &original)
        .map_err(|_| "resolve original owner")?;
    let models = ArchiveModels::new(archive).with_model_node_sources();
    let assembly = Assembly::build(&loadout, &models, &AssemblyOptions::default())
        .map_err(|_| "assemble original owner")?;
    let owner = assembly.parts().first().ok_or("source owner is missing")?;
    let glow = assembly
        .entity_glow(0, &input, &forge)
        .map_err(|_| "join installed Glow declarations")?;
    check(
        glow.authored.record_index == record.index() && glow.authored.guid == record.guid(),
        "the Glow projection identifies a different Record",
    )?;
    check(
        glow.links.len() == expected.len(),
        "Glow link census differs",
    )?;
    let DataForgeOrigin::Archive(original_input) = input.origin() else {
        return Err("DataForge input has no exact Archive Entry".into());
    };
    let primary = PrimaryModel::read(archive, owner)?;
    let mut named_binding = false;
    for (link, authored) in glow.links.iter().zip(&expected) {
        verify_authored(&glow.authored, link, authored, original_input.encryption())?;
        let subsets = verify_selection(owner, &primary, link)?;
        named_binding |= subsets > 0
            && authored
                .selector
                .is_some_and(|name| !matches!(name, "" | "*"));
    }
    check(
        named_binding,
        "the chosen source owner has no named bound Glow subset",
    )
}

struct AuthoredLink<'source> {
    component: usize,
    name: &'static str,
    kind: GlowEffectKind,
    effect: usize,
    link: usize,
    group_enabled: Option<bool>,
    effect_enabled: Option<bool>,
    selector: Option<&'source str>,
}

fn authored_links<'source>(root: &StructValue<'source>) -> Vec<AuthoredLink<'source>> {
    let mut sites = Vec::new();
    for (component, value) in array(root, "Components").iter().enumerate() {
        let Value::Struct(value) = value else {
            continue;
        };
        if value.type_name() != "SEntityComponentEffects" {
            continue;
        }
        let Some(Value::Struct(effects)) = value.field("entityEffects") else {
            continue;
        };
        let Some(Value::Struct(group)) = effects.field("materialEffects") else {
            continue;
        };
        for (name, kind) in [
            ("tagEffects", GlowEffectKind::Tag),
            ("triggerEffects", GlowEffectKind::Trigger),
        ] {
            append_links(component, group, name, kind, &mut sites);
        }
    }
    sites
}

fn append_links<'source>(
    component: usize,
    group: &StructValue<'source>,
    name: &'static str,
    kind: GlowEffectKind,
    sites: &mut Vec<AuthoredLink<'source>>,
) {
    for (effect, value) in array(group, name).iter().enumerate() {
        let Value::Struct(value) = value else {
            continue;
        };
        for (link, row) in array(value, "propertyLinks").iter().enumerate() {
            let Value::Struct(row) = row else {
                continue;
            };
            if text(row.field("materialProperty")) == Some("Glow") {
                sites.push(AuthoredLink {
                    component,
                    name,
                    kind,
                    effect,
                    link,
                    group_enabled: enabled(group),
                    effect_enabled: enabled(value),
                    selector: text(row.field("subMeshName")),
                });
            }
        }
    }
}

fn array<'tree, 'source>(
    value: &'tree StructValue<'source>,
    name: &str,
) -> &'tree [Value<'source>] {
    match value.field(name) {
        Some(Value::Array(rows)) => rows,
        _ => &[],
    }
}

const fn text<'source>(value: Option<&Value<'source>>) -> Option<&'source str> {
    match value {
        Some(Value::String(text) | Value::Enum(text)) => Some(text),
        _ => None,
    }
}

fn enabled(value: &StructValue<'_>) -> Option<bool> {
    match value.field("enabled") {
        Some(Value::Bool(enabled)) => Some(*enabled),
        _ => None,
    }
}

fn verify_authored(
    source: &p4kx_extraction::EntityMaterialEffects,
    link: &EntityGlowLink,
    expected: &AuthoredLink<'_>,
    encryption: SourceEncryption,
) -> TestResult {
    check(
        link.component == expected.component
            && link.kind == expected.kind
            && link.effect == expected.effect
            && link.link == expected.link
            && link.selector.as_deref() == expected.selector
            && link.group_enabled == expected.group_enabled
            && link.effect_enabled == expected.effect_enabled,
        "Glow declaration differs from its authored occurrence",
    )?;
    let component = source
        .components
        .iter()
        .find(|row| row.component == expected.component)
        .ok_or("missing physical component declaration")?;
    let path = FieldPath::parse(&format!(
        "/{}/{}/propertyLinks/{}/materialProperty",
        expected.name, expected.effect, expected.link
    ))?;
    let origin = component
        .field_origins
        .get(&path)
        .ok_or("missing Glow property coordinates")?;
    let encrypted = match encryption {
        SourceEncryption::Encrypted => Some(true),
        SourceEncryption::Unencrypted => Some(false),
        _ => None,
    };
    check(
        origin.source_set_complete()
            && origin.contains_encrypted_source() == encrypted
            && source.provenance.uses().iter().any(|used| {
                origin.source_uses_refs().contains(used.id())
                    && matches!(used.location(), SourceLocation::Subresource { namespace, field, .. }
                        if namespace == "dataforgeDecodedBytes" && field == "arrayHeader")
            }),
        "original Glow property source evidence differs",
    )
}

fn verify_selection(
    owner: &Part,
    primary: &PrimaryModel<'_>,
    link: &EntityGlowLink,
) -> TestResult<usize> {
    let target = match link.selector.as_deref() {
        None => {
            check(
                matches!(link.selection, GlowSelection::Absent),
                "absent Glow selector changed",
            )?;
            return Ok(0);
        }
        Some("" | "*") => None,
        Some(selector) => match primary.matching(selector)?.as_slice() {
            [] => {
                check(
                    matches!(link.selection, GlowSelection::Unmatched),
                    "unmatched selector changed",
                )?;
                return Ok(0);
            }
            [node] => Some(*node),
            expected => {
                check(
                    matches!(&link.selection, GlowSelection::Ambiguous { nodes } if nodes == expected),
                    "ambiguous source node set changed",
                )?;
                return Ok(0);
            }
        },
    };
    let expected = subsets(primary.model, target);
    if expected.is_empty() {
        check(
            matches!(link.selection, GlowSelection::NoDrawableSubsets),
            "empty subset result changed",
        )?;
        return Ok(0);
    }
    let GlowSelection::Bound(published) = &link.selection else {
        return Err("a drawable owner subset did not bind".into());
    };
    check(
        published.len() == expected.len(),
        "bound subset count differs",
    )?;
    for (published, original) in published.iter().zip(&expected) {
        verify_subset(owner, primary.nodes, published, original)?;
    }
    Ok(published.len())
}

struct OriginalSubset {
    lod: u32,
    node: usize,
    subset: usize,
    source_node_id: u16,
    indices: Range<usize>,
    source_slot: u16,
}

fn subsets(model: &Model, target: Option<usize>) -> Vec<OriginalSubset> {
    let mut result = Vec::new();
    for lod in &model.lods {
        for mesh in &lod.meshes {
            if target.is_some_and(|node| node != mesh.node) {
                continue;
            }
            for (ordinal, subset) in mesh.submeshes.iter().enumerate() {
                let Some(end) = subset.first_index.checked_add(subset.index_count) else {
                    continue;
                };
                if mesh.submesh_indices(subset).is_some() {
                    result.push(OriginalSubset {
                        lod: lod.level,
                        node: mesh.node,
                        subset: ordinal,
                        source_node_id: subset.node_id,
                        indices: subset.first_index..end,
                        source_slot: subset.material_id,
                    });
                }
            }
        }
    }
    result
}

fn verify_subset(
    owner: &Part,
    nodes: &ModelNodeSources,
    output: &GlowSubset,
    expected: &OriginalSubset,
) -> TestResult {
    let node_source = nodes
        .node(expected.node)
        .and_then(|node| Some((node.chunk(), node.name()?)));
    let material = owner
        .materials()
        .library()
        .filter(|library| !library.is_empty_slot(expected.source_slot))
        .and_then(|library| library.material(expected.source_slot));
    check(
        output.lod == expected.lod
            && output.node == expected.node
            && output.subset == expected.subset
            && output.source_node_id == expected.source_node_id
            && output.indices == expected.indices
            && output.node_name_source == node_source
            && output.material.source_slot == expected.source_slot
            && output.material.resolves == material.is_some()
            && output.material.shader.as_deref() == material.map(|slot| slot.shader.as_str()),
        "bound subset or material slot differs from the original owner",
    )
}

fn check(condition: bool, message: &'static str) -> TestResult {
    if condition {
        Ok(())
    } else {
        Err(message.into())
    }
}
