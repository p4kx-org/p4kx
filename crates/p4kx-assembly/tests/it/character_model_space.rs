// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{
    ArchiveModels, Assembly, AssemblyOptions, AttachedModel, CdfAttachmentField, CdfAttachmentSite,
    LoadoutResolver,
};
use p4kx_extraction::{Archive, DataForgeEntry, SourceLocation, SourceUseRole};
use p4kx_gltf::GlbOptions;
use p4kx_testkit::{BONE_THRUSTER, bone_thruster_definition, bone_thruster_entries, check_glb};

use super::assert_near;
use crate::assembly_support::{TestResult, fixture};

const ATTACHMENTS: &str = r#"
<Attachment Type="CA_BONE" AName="Model" BoneName="tip" Binding="Objects\Test\housing.cgf"
    Position="1,2,3" Rotation="1,0,0,0"/>
<Attachment Type="CA_BONE" AName="Mixed" BoneName="tip" Binding="Objects\Test\housing.cgf"
    Position="4,5,6" RelRotation="1,0,0,0" Rotation="invalid shadowed rotation"/>
<Attachment Type="CA_BONE" AName="Default" BoneName="tip" Binding="Objects\Test\housing.cgf"
    RelRotation="1,0,0,0" Rotation="presence selects model default"/>
"#;

#[test]
fn model_space_and_mixed_pose_place_the_meshes_from_the_original_operands() -> TestResult {
    let definition = bone_thruster_definition(ATTACHMENTS);
    let assembly = build(&definition)?;
    let [model, mixed, default] = assembly.root().ok_or("a root part")?.attachments() else {
        return Err("all original rigid bindings are preserved".into());
    };
    assert_eq!(model.name(), "Model", "original occurrence order");
    assert_eq!(mixed.name(), "Mixed", "original occurrence order");
    check_model_space(model);
    check_mixed_space(&definition, mixed)?;
    check_default_selector(&definition, default)?;
    check_glb_nodes(&assembly)
}

fn build(definition: &str) -> Result<Assembly, Box<dyn std::error::Error>> {
    let file = fixture(bone_thruster_entries(definition))?;
    let archive = Archive::open(&file.path)?;
    let models = ArchiveModels::new(&archive).with_model_node_sources();
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let mut resolver = LoadoutResolver::new(&forge);
    let record = resolver.entities().find(BONE_THRUSTER)?;
    let tree = resolver.resolve(&record)?;
    Ok(Assembly::build(
        &tree,
        &models,
        &AssemblyOptions::default(),
    )?)
}

fn check_model_space(attachment: &AttachedModel) {
    assert!(
        attachment.is_bone_found(),
        "the fixture has an original bone"
    );
    assert!(
        !attachment.placement_uses_bone_pose(),
        "the model-space pose is independent of the bind frame"
    );
    assert_near(attachment.local().translation(), [1.0, 2.0, 3.0]);
    assert_eq!(
        attachment
            .placement_cdf_sources()
            .map(|(source, role)| (source.field(), role))
            .collect::<Vec<_>>(),
        [
            (CdfAttachmentField::Type, SourceUseRole::Selector),
            (CdfAttachmentField::Binding, SourceUseRole::Selector),
            (CdfAttachmentField::BoneName, SourceUseRole::Selector),
            (CdfAttachmentField::ModelPosition, SourceUseRole::Conversion),
            (CdfAttachmentField::ModelRotation, SourceUseRole::Conversion),
        ],
        "the named bone admits the attachment and the model-space fields supply its frame"
    );
}

fn check_mixed_space(definition: &str, attachment: &AttachedModel) -> TestResult {
    assert!(
        attachment.placement_uses_bone_pose(),
        "the selected relative orientation uses the bind frame"
    );
    assert_near(attachment.local().translation(), [4.0, 5.0, 6.0]);
    let [[x, _, _, _], [y, _, _, _], [z, _, _, _]] = attachment.local().rows;
    assert_near([x, y, z], [0.0, 1.0, 0.0]);
    let all_fields: Vec<_> = attachment
        .cdf_sources()
        .iter()
        .filter(|site| site.field().is_offset())
        .map(CdfAttachmentSite::field)
        .collect();
    assert_eq!(
        all_fields,
        [
            CdfAttachmentField::RelativeRotation,
            CdfAttachmentField::ModelPosition,
            CdfAttachmentField::ModelRotation,
        ],
        "the original shadowed attribute is still addressable"
    );
    let attributes = attachment
        .placement_cdf_sources()
        .filter(|(_, role)| *role == SourceUseRole::Conversion)
        .map(|(site, _)| original_attribute(definition, site))
        .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(
        attributes,
        [r#"RelRotation="1,0,0,0""#, r#"Position="4,5,6""#],
        "conversion cites only the exact selected authored values"
    );
    Ok(())
}

fn check_default_selector(definition: &str, attachment: &AttachedModel) -> TestResult {
    assert_near(attachment.local().translation(), [0.0; 3]);
    let fields = attachment
        .placement_cdf_sources()
        .filter(|(site, _)| site.field().is_offset())
        .map(|(site, role)| Ok((original_attribute(definition, site)?, role)))
        .collect::<Result<Vec<_>, Box<dyn std::error::Error>>>()?;
    assert_eq!(
        fields,
        [
            (r#"RelRotation="1,0,0,0""#, SourceUseRole::Conversion),
            (
                r#"Rotation="presence selects model default""#,
                SourceUseRole::Selector
            ),
        ],
        "the shadowed model attribute selects the default frame without supplying a rotation"
    );
    Ok(())
}

fn original_attribute<'a>(
    definition: &'a str,
    site: &CdfAttachmentSite,
) -> Result<&'a str, Box<dyn std::error::Error>> {
    let Some(SourceLocation::Subresource {
        namespace, indices, ..
    }) = site.location()
    else {
        return Err("an observed CDF source attribute".into());
    };
    assert_eq!(
        namespace, "cdfAttachmentTextBytes",
        "source coordinate kind"
    );
    let [start, end] = indices.as_slice() else {
        return Err("a half-open original attribute span".into());
    };
    definition
        .get(usize::try_from(*start)?..usize::try_from(*end)?)
        .ok_or_else(|| "a range in the original attachment declaration".into())
}

fn check_glb_nodes(assembly: &Assembly) -> TestResult {
    let mut bytes = Vec::new();
    assembly.write_glb(&GlbOptions::default(), &mut bytes)?;
    let glb = check_glb(&bytes)?;
    let owner = glb
        .document
        .nodes()
        .find(|node| node.name() == Some(BONE_THRUSTER))
        .ok_or("the GLB root part")?;
    for (name, expected) in [("Model", [1.0, 3.0, -2.0]), ("Mixed", [4.0, 6.0, -5.0])] {
        let node = owner
            .children()
            .find(|node| node.name() == Some(name))
            .ok_or("the emitted rigid child")?;
        assert_near(node.transform().decomposed().0, expected);
    }
    Ok(())
}
