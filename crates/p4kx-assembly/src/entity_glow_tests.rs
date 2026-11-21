// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_extraction::{Archive, DataForgeEntry};
use p4kx_model::MaterialStatus;
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestInstance, TestModel, TestNode, TestRecord,
    TestStruct, TestType, TestValue, entity_schema,
};

use crate::{ArchiveModels, Assembly, AssemblyOptions, LoadoutResolver, ModelProvider};

use super::{Binding, EntityGlowBindingError, GlowEffectKind, GlowSelection};

const MODEL: &str = "Objects/Test/glow.cgf";
const ENTITY: &str = "EntityClassDefinition";
const EFFECTS: &str = "SEntityComponentEffects";

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn exact_source_binding_retains_disabled_links_duplicate_node_names_and_original_primitive_slots()
-> TestResult {
    let folder = tempfile::tempdir()?;
    let p4k = folder.path().join("Data.p4k");
    let model = model("Other");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(r"Data\Game2.dcb", database()))
        .entry(TestEntry::stored(format!("Data/{MODEL}"), model.cgf()))
        .entry(TestEntry::stored(
            "Data/Objects/Test/glow.cgfm",
            model.cgfm(),
        ))
        .write_to(&p4k)?;
    let archive = Archive::open(&p4k)?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let mut resolver = LoadoutResolver::new(&forge).with_forge_entry(&entry);
    let record = resolver.entities().find("SyntheticGlow")?;
    let tree = resolver.resolve(&record)?;
    let models = ArchiveModels::new(&archive).with_model_node_sources();
    let assembly = Assembly::build(&tree, &models, &AssemblyOptions::default())?;
    let glow = assembly.entity_glow(0, &entry, &forge)?;
    let [tag, trigger] = glow.links.as_slice() else {
        return Err("two independent original links".into());
    };
    assert_eq!(
        tag.kind,
        GlowEffectKind::Tag,
        "the disabled tag is retained"
    );
    assert_eq!(tag.group_enabled, Some(false), "source enablement");
    assert_eq!(tag.effect_enabled, Some(false), "effect enablement");
    assert_eq!(
        tag.selector.as_deref(),
        Some("DRAWABLE"),
        "authored names stay unchanged"
    );
    assert_eq!(
        tag.selector_origin
            .as_ref()
            .and_then(p4kx_extraction::FieldOrigin::contains_encrypted_source),
        Some(false),
        "unambiguously Plain DataForge selector evidence",
    );
    assert_eq!(
        trigger.kind,
        GlowEffectKind::Trigger,
        "no merging separate contributions"
    );
    verify_named_subset(&tag.selection)?;
    let another_entry = DataForgeEntry::read(&archive)?;
    assert!(
        matches!(
            assembly.entity_glow(0, &another_entry, &forge),
            Err(EntityGlowBindingError::SourceIdentity)
        ),
        "matching decoded bytes do not authorize a different input owner"
    );
    Ok(())
}

#[test]
fn repeated_original_names_remain_ambiguous_even_when_one_has_no_subsets() -> TestResult {
    let folder = tempfile::tempdir()?;
    let p4k = folder.path().join("Data.p4k");
    let model = model("drawable");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(format!("Data/{MODEL}"), model.cgf()))
        .entry(TestEntry::stored(
            "Data/Objects/Test/glow.cgfm",
            model.cgfm(),
        ))
        .write_to(&p4k)?;
    let archive = Archive::open(&p4k)?;
    let loaded = ArchiveModels::new(&archive)
        .with_model_node_sources()
        .model(MODEL, 0)?;
    let binding = Binding::of(
        Some(&loaded.model),
        &MaterialStatus::Unnamed,
        &loaded.archive_sources,
    );
    assert_eq!(
        binding.select(Some("DRAWABLE")),
        GlowSelection::Ambiguous { nodes: vec![1, 2] },
        "matching node identities stay distinct",
    );
    assert!(
        matches!(binding.select(Some("*")), GlowSelection::Bound(_)),
        "wildcard addresses the direct owner"
    );
    assert_eq!(
        binding.select(Some("missing")),
        GlowSelection::Unmatched,
        "no fuzzy match"
    );
    assert_eq!(
        binding.select(None),
        GlowSelection::Absent,
        "absence remains explicit"
    );
    assert_eq!(
        binding.select(Some("root")),
        GlowSelection::NoDrawableSubsets,
        "a named empty root cannot supply geometry"
    );
    Ok(())
}

fn verify_named_subset(selection: &GlowSelection) -> TestResult {
    let GlowSelection::Bound(rows) = selection else {
        return Err("actual model node must be bound".into());
    };
    assert_eq!(rows.len(), 2, "the fixture's two drawable material subsets");
    for (index, row) in rows.iter().enumerate() {
        assert_eq!(row.node, 1, "observed owning node");
        assert_eq!(row.lod, 0, "selected model level");
        assert_eq!(row.subset, index, "physical subset position");
        assert!(
            row.node_name_source.is_some(),
            "retained original NMC name coordinates"
        );
        assert_eq!(
            row.material.source_slot,
            u16::try_from(index + 1)?,
            "source material ID"
        );
    }
    Ok(())
}

fn model(last_name: &str) -> TestModel {
    let identity = [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
    ];
    let node = |name: &str, parent, kind| TestNode {
        name: name.to_owned(),
        parent,
        local: identity,
        kind,
    };
    TestModel::cube().with_nodes(vec![
        node("root", None, 0),
        node("Drawable", Some(0), 0),
        node(last_name, Some(0), 2),
    ])
}

fn database() -> Vec<u8> {
    let geometry = value(
        TestInstance::new("SGeometryResourceParams").value(
            "Geometry",
            value(
                TestInstance::new("SGeometryNodeParams").value(
                    "Geometry",
                    value(
                        TestInstance::new("SGeometryDataParams").value(
                            "Geometry",
                            value(
                                TestInstance::new("GlobalResourceGeometry")
                                    .value("path", TestValue::text(MODEL)),
                            ),
                        ),
                    ),
                ),
            ),
        ),
    );
    let field = |name: &str| TestType::Class(name.to_owned());
    entity_schema(DataForgeBuilder::new())
        .structure(
            TestStruct::new(EFFECTS)
                .parent("DataForgeComponentParams")
                .field("entityEffects", field("Effects")),
        )
        .structure(TestStruct::new("Effects").field("materialEffects", field("MaterialEffects")))
        .structure(
            TestStruct::new("MaterialEffects")
                .field("enabled", TestType::Bool)
                .array("tagEffects", field("Effect"))
                .array("triggerEffects", field("Effect")),
        )
        .structure(
            TestStruct::new("Effect")
                .field("enabled", TestType::Bool)
                .array("propertyLinks", field("PropertyLink")),
        )
        .structure(
            TestStruct::new("PropertyLink")
                .field("materialProperty", TestType::String)
                .field("subMeshName", TestType::String),
        )
        .record(
            TestRecord::new(
                42,
                ENTITY,
                "EntityClassDefinition.SyntheticGlow",
                "synthetic.xml",
            )
            .value("Components", TestValue::Array(vec![geometry, effects()])),
        )
        .build()
}

fn effects() -> TestValue {
    let effect = |selector: &str, enabled: bool| {
        value(
            TestInstance::new("Effect")
                .value("enabled", TestValue::Bool(enabled))
                .value(
                    "propertyLinks",
                    TestValue::Array(vec![value(
                        TestInstance::new("PropertyLink")
                            .value("materialProperty", TestValue::text("Glow"))
                            .value("subMeshName", TestValue::text(selector)),
                    )]),
                ),
        )
    };
    let groups = value(
        TestInstance::new("MaterialEffects")
            .value("enabled", TestValue::Bool(false))
            .value(
                "tagEffects",
                TestValue::Array(vec![effect("DRAWABLE", false)]),
            )
            .value("triggerEffects", TestValue::Array(vec![effect("*", true)])),
    );
    value(TestInstance::new(EFFECTS).value(
        "entityEffects",
        value(TestInstance::new("Effects").value("materialEffects", groups)),
    ))
}

fn value(instance: TestInstance) -> TestValue {
    TestValue::Struct(instance)
}
