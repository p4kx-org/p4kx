// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{
    ArchiveModels, Assembly, AssemblyOptions, CdfAttachmentField, CdfAttachmentSite,
    LoadoutResolver, ModelFailure, ModelProvider, Part, ProvidedModel, SkeletonProblem,
};
use p4kx_extraction::model::Lod;
use p4kx_extraction::{Archive, DataForgeEntry, SourceLocation};
use p4kx_gltf::GlbOptions;
use p4kx_testkit::{
    BONE_THRUSTER as THRUSTER, TestEntry, TestModel, TestSkeleton, bone_thruster_entries, check_glb,
};

use super::{assert_near, draws};
use crate::assembly_support::{TestResult, fixture};

const RIGID_CDF: &str = r#"<CharacterDefinition>
  <Model File="Objects\Test\rig.CGA"/>
  <AttachmentList>
    <Attachment Type="CA_BONE" AName="Socket" BoneName="root" Binding=""/>
    <Attachment Type="CA_SKIN" AName="UnboundSkin"/>
    <Attachment Type="CA_BONE" AName="Housing" BoneName="tip" Binding="Objects\Test\housing.cgf"
      Material="Objects\Test\paint.mtl" RelPosition="1,0,0" RelRotation="1,0,0,0"/>
  </AttachmentList>
</CharacterDefinition>"#;
const RIG_MODEL: &str = r"Data\Objects\Test\rig.cga";
const RIG_GEOMETRY: &str = r"Data\Objects\Test\rig.cgam";

#[test]
fn a_declared_cga_draws_the_body_and_places_its_only_bound_attachment() -> TestResult {
    let body = TestModel::cube()
        .with_compact_skeleton(&TestSkeleton::two_bones())
        .with_material_library("Objects/Test/paint");
    let mut entries = bone_thruster_entries(RIGID_CDF);
    entries.extend([
        TestEntry::stored(RIG_MODEL, body.cgf()),
        TestEntry::stored(RIG_GEOMETRY, body.cgfm()),
    ]);
    let fixture = fixture(entries)?;
    let archive = Archive::open(&fixture.path)?;
    let models = ArchiveModels::new(&archive).with_model_node_sources();
    let direct = models.model("Objects/Test/rig.cga", 0)?;
    assert_eq!(
        direct.model.lods.first().map(Lod::vertex_count),
        Some(24),
        "the declared file has its own drawable geometry"
    );

    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let mut resolver = LoadoutResolver::new(&forge);
    let record = resolver.entities().find(THRUSTER)?;
    let tree = resolver.resolve(&record)?;
    let assembly = Assembly::build(&tree, &models, &AssemblyOptions::default())?;

    let root = assembly.root().ok_or("the root part")?;
    verify_rigid_model(root)?;
    let [attached] = root.attachments() else {
        return Err("the original bound housing is the only drawn attachment".into());
    };
    assert_eq!(attached.name(), "Housing", "the original attachment name");
    assert_eq!(attached.bone(), "tip", "the original bone binding");
    assert_eq!(
        attached.geometry(),
        r"Data\Objects\Test\housing.cgf",
        "the bound attachment input"
    );
    assert!(
        attached.is_bone_found(),
        "the body supplies the attachment bone"
    );
    assert_near(attached.local().translation(), [0.0, 1.0, 2.0]);
    let binding = attached
        .cdf_sources()
        .iter()
        .find(|site| site.field() == CdfAttachmentField::Binding)
        .and_then(CdfAttachmentSite::location)
        .ok_or("the original attachment binding")?;
    verify_source_attribute(
        &binding,
        "cdfAttachmentTextBytes",
        "Binding",
        r#"Binding="Objects\Test\housing.cgf""#,
    )?;
    assert!(assembly.issues().is_empty(), "{:?}", assembly.issues());
    verify_rigid_glb(&assembly)
}

fn verify_rigid_model(part: &Part) -> TestResult {
    let model = part.model().ok_or("the declared body")?;
    assert_eq!(
        model
            .bones
            .iter()
            .map(|bone| bone.name.as_str())
            .collect::<Vec<_>>(),
        ["root", "tip"],
        "the body retains its attached and unbound bones"
    );
    let sources: Vec<_> = part
        .model_archive_sources()
        .iter()
        .filter_map(|source| source.origin().map(|origin| origin.entry().path()))
        .collect();
    assert_eq!(sources, [RIG_MODEL, RIG_GEOMETRY], "the actual body inputs");
    assert_eq!(
        part.model_definition_origin()
            .map(|origin| origin.entry().path()),
        Some(r"Data\Objects\Test\thruster.cdf"),
        "the authoritative CDF input"
    );
    let selectors: Vec<_> = part.cdf_model_selectors().collect();
    let [selector] = selectors.as_slice() else {
        return Err("only the declared Model File selects the body".into());
    };
    verify_source_attribute(
        selector,
        "cdfModelTextBytes",
        "File",
        r#"File="Objects\Test\rig.CGA""#,
    )
}

fn verify_source_attribute(
    source: &SourceLocation,
    expected_namespace: &str,
    expected_name: &str,
    expected_attribute: &str,
) -> TestResult {
    let SourceLocation::Subresource {
        namespace,
        indices,
        field,
    } = source
    else {
        return Err("an original CDF attribute location".into());
    };
    let [start, end] = indices.as_slice() else {
        return Err("a half-open original CDF byte range".into());
    };
    assert_eq!(namespace, expected_namespace, "the CDF attribute role");
    assert_eq!(field, expected_name, "the original attribute name");
    let range = usize::try_from(*start)?..usize::try_from(*end)?;
    assert_eq!(
        RIGID_CDF.get(range),
        Some(expected_attribute),
        "the locator addresses the authored attribute bytes"
    );
    Ok(())
}

fn verify_rigid_glb(assembly: &Assembly) -> TestResult {
    let mut bytes = Vec::new();
    assembly.write_glb(&GlbOptions::default(), &mut bytes)?;
    let glb = check_glb(&bytes)?;
    let root = glb
        .document
        .nodes()
        .find(|node| node.name() == Some(THRUSTER))
        .ok_or("the root part node")?;
    let housing = root
        .children()
        .find(|node| node.name() == Some("Housing"))
        .ok_or("the housing child node")?;
    assert!(draws(&housing), "the bound attachment draws separately");
    assert!(
        root.mesh().is_some()
            || root
                .children()
                .filter(|node| node.name() != Some("Housing"))
                .any(|node| draws(&node)),
        "the declared body draws on its own"
    );
    Ok(())
}

fn provided_rigid(
    definition: &str,
    extra: impl IntoIterator<Item = TestEntry>,
) -> Result<ProvidedModel, Box<dyn std::error::Error>> {
    let mut entries = bone_thruster_entries(definition);
    entries.extend(extra);
    let file = fixture(entries)?;
    let archive = Archive::open(&file.path)?;
    Ok(ArchiveModels::new(&archive).model("Objects/Test/thruster.cdf", 0)?)
}

#[test]
fn a_drawable_declared_cgf_also_selects_the_body() -> TestResult {
    let body = TestModel::cube();
    let definition = RIGID_CDF.replace("rig.CGA", "rig.cgf");
    let provided = provided_rigid(
        &definition,
        [
            TestEntry::stored(r"Data\Objects\Test\rig.cgf", body.cgf()),
            TestEntry::stored(r"Data\Objects\Test\rig.cgfm", body.cgfm()),
        ],
    )?;
    assert_eq!(
        provided.entry, r"Data\Objects\Test\rig.cgf",
        "geometry selects the authored body"
    );
    assert_eq!(
        provided.attachments.len(),
        1,
        "the original attachment remains"
    );
    Ok(())
}

#[test]
fn a_cga_with_only_a_rig_keeps_the_first_bound_fallback() -> TestResult {
    let rig = TestModel::cube()
        .helpers_only()
        .with_compact_skeleton(&TestSkeleton::two_bones());
    let provided = provided_rigid(RIGID_CDF, [TestEntry::stored(RIG_MODEL, rig.cgf())])?;
    assert_eq!(
        provided.entry, r"Data\Objects\Test\housing.cgf",
        "the first bound fallback supplies geometry for a rig-only declaration"
    );
    assert!(
        provided.attachments.is_empty(),
        "the fallback is never duplicated"
    );
    assert!(provided.skeleton.is_none(), "the declared rig is readable");
    Ok(())
}

#[test]
fn a_rig_only_cga_places_every_rigid_binding_from_the_original_rig() -> TestResult {
    let rig = TestModel::cube()
        .helpers_only()
        .with_compact_skeleton(&TestSkeleton::two_bones());
    let definition = RIGID_CDF
        .replace(r#"RelRotation="1,0,0,0""#, r#"RelRotation="0,1,0,0""#)
        .replace(
            "</AttachmentList>",
            r#"<Attachment Type="CA_BONE" AName="Spare"
            BoneName="root" Binding="Objects\Test\housing.cgf" RelPosition="0,3,0"/>
            </AttachmentList>"#,
        );
    let mut entries = bone_thruster_entries(&definition);
    entries.push(TestEntry::stored(RIG_MODEL, rig.cgf()));
    let fixture = fixture(entries)?;
    let archive = Archive::open(&fixture.path)?;
    let models = ArchiveModels::new(&archive).with_model_node_sources();
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let mut resolver = LoadoutResolver::new(&forge);
    let record = resolver.entities().find(THRUSTER)?;
    let tree = resolver.resolve(&record)?;
    let assembly = Assembly::build(&tree, &models, &AssemblyOptions::default())?;

    let root = assembly.root().ok_or("the declared rig owner")?;
    verify_rig_only_bindings(root)?;
    assert!(assembly.issues().is_empty(), "{:?}", assembly.issues());
    verify_rig_only_glb(&assembly)
}

fn verify_rig_only_bindings(root: &Part) -> TestResult {
    let rig = root.model().ok_or("the original rig")?;
    assert!(
        rig.lods.iter().all(|lod| lod.meshes.is_empty()),
        "the rig has no drawable body"
    );
    let [first, second] = root.attachments() else {
        return Err("both rigid bindings, once each, in original order".into());
    };
    assert_eq!(
        (first.name(), second.name()),
        ("Housing", "Spare"),
        "the attachments retain CDF order"
    );
    assert!(
        first.is_bone_found() && second.is_bone_found(),
        "both bind to the declared rig"
    );
    assert_near(first.local().translation(), [0.0, 1.0, 2.0]);
    assert_near(second.local().translation(), [0.0, 3.0, 0.0]);
    for (actual, expected) in first.local().rows.iter().zip([
        [0.0, 0.0, -1.0, 0.0],
        [1.0, 0.0, 0.0, 1.0],
        [0.0, -1.0, 0.0, 2.0],
    ]) {
        for (actual, expected) in actual.iter().zip(expected) {
            assert!(
                (actual - expected).abs() < 1e-5,
                "the CDF rotation is relative to the bone"
            );
        }
    }
    let source = root
        .verified_attachment_bone_placement_source(0)
        .ok_or("the first bone source")?;
    assert_eq!(
        source.origin().entry().path(),
        RIG_MODEL,
        "the bone was read from the declared rig"
    );
    assert_eq!(
        source.bone_ordinal(),
        1,
        "the placement identifies the authored tip bone"
    );
    assert_eq!(
        first
            .material_archive_origins()
            .library()
            .map(|origin| origin.entry().path()),
        Some(r"Data\Objects\Test\paint.mtl"),
        "the first binding retains its CDF material"
    );
    Ok(())
}

fn verify_rig_only_glb(assembly: &Assembly) -> TestResult {
    let mut output = Vec::new();
    assembly.write_glb(&GlbOptions::default(), &mut output)?;
    let glb = check_glb(&output)?;
    let root = glb
        .document
        .nodes()
        .find(|node| node.name() == Some(THRUSTER))
        .ok_or("the GLB rig owner")?;
    assert!(root.mesh().is_none(), "the rig owner has no mesh");
    for (name, translation) in [("Housing", [0.0, 2.0, -1.0]), ("Spare", [0.0, 0.0, -3.0])] {
        let mut matches = root.children().filter(|node| node.name() == Some(name));
        let node = matches.next().ok_or("the rigid GLB child")?;
        assert!(matches.next().is_none(), "{name} is emitted once");
        assert!(draws(&node), "{name} draws its bound mesh");
        assert_near(node.transform().decomposed().0, translation);
    }
    Ok(())
}

#[test]
fn the_first_bound_skin_wins_over_a_drawable_declared_file() -> TestResult {
    let body = TestModel::cube();
    let definition = RIGID_CDF.replace(
        "</AttachmentList>",
        r#"<Attachment Type="CA_SKIN" Binding="Objects\Test\nozzle.skin" Material="Objects\Test\paint.mtl"/>
  </AttachmentList>"#,
    );
    let provided = provided_rigid(
        &definition,
        [
            TestEntry::stored(RIG_MODEL, body.cgf()),
            TestEntry::stored(RIG_GEOMETRY, body.cgfm()),
        ],
    )?;
    assert_eq!(
        provided.entry, r"Data\Objects\Test\nozzle.skin",
        "a bound skin takes precedence"
    );
    assert_eq!(
        provided.model.material_library.as_deref(),
        Some("Objects/Test/paint"),
        "the skin material override is kept"
    );
    assert_eq!(
        provided.attachments.len(),
        1,
        "the bound housing stays attached"
    );
    Ok(())
}

#[test]
fn a_missing_declared_file_preserves_the_binding_and_missing_skeleton_diagnostic() -> TestResult {
    let provided = provided_rigid(RIGID_CDF, [])?;
    assert_eq!(
        provided.entry, r"Data\Objects\Test\housing.cgf",
        "the original fallback remains usable"
    );
    assert!(
        matches!(provided.skeleton, Some(SkeletonProblem::Missing { path }) if path == "Objects/Test/rig.CGA"),
        "missing original declaration is reported independently"
    );
    Ok(())
}

#[test]
fn a_declared_model_requiring_a_key_remains_fatal() -> TestResult {
    let body = TestEntry::stored(RIG_MODEL, TestModel::cube().cgf()).encrypted();
    let error = provided_rigid(RIGID_CDF, [body])
        .err()
        .ok_or("the declared file requires an Archive key")?;
    assert!(
        matches!(
            error.downcast_ref::<ModelFailure>(),
            Some(ModelFailure::Fatal(_))
        ),
        "the optional geometry read preserves a fatal Archive error"
    );
    Ok(())
}
