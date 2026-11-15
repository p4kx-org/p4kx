// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Original coordinates for flattened Prefab objects.

use p4kx_cryxml::SourceAttributePosition;
use p4kx_model::{
    PrefabAttributeObservation, PrefabLibrary, PrefabModelSource, PrefabObjectSource,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn attribute<'a>(
    text: &'a str,
    original: Option<&SourceAttributePosition>,
) -> Result<&'a str, &'static str> {
    let Some(SourceAttributePosition::Text(range)) = original else {
        return Err("missing text attribute");
    };
    text.get(range.clone()).ok_or("attribute range must fit")
}

#[test]
fn object_sources_follow_geometry_selection_and_the_actual_group_operands() -> TestResult {
    let text = concat!(
        "<PrefabsLibrary><Prefab Name='owner'><Objects>",
        "<Object Type='Group' Id='a' Pos='1,2,3'><Objects>",
        "<Object Type='Entity' Name='one' Geometry=' ' Parent='a' AttachmentTarget=''>",
        "<PropertiesDataCore><EntityGeometryResource><Geometry><Geometry><Geometry path='v.cgf'/>",
        "</Geometry></Geometry></EntityGeometryResource></PropertiesDataCore></Object>",
        "<Object Type='Brush' Name='two' Prefab='b.cgf' Geometry='ignored' Parent='external'",
        " AttachmentTarget='not-read' Scale='2,2,2'/>",
        "</Objects></Object></Objects></Prefab></PrefabsLibrary>",
    );
    let parsed = PrefabLibrary::parse_positioned_objects(text.as_bytes())?;
    let details = PrefabLibrary::parse_positioned(text.as_bytes())?;
    assert_eq!(parsed.library, details.library);
    let detail = details.source.ok_or("detail positions")?;
    let detail = detail.prefabs.first().ok_or("detail row positions")?;
    assert!(detail.object_sources.is_none());

    let sources = parsed.source.ok_or("library positions")?;
    let row = sources.prefabs.first().ok_or("prefab positions")?;
    let positions = row.object_sources.as_deref().ok_or("object positions")?;
    let [resource, brush] = positions else {
        return Err("expected two flattened objects".into());
    };
    let PrefabModelSource::Resource { geometry, path, .. } = &resource.model else {
        return Err("expected entity resource geometry".into());
    };
    assert_eq!(attribute(text, geometry.as_ref())?, "Geometry=' '");
    assert_eq!(attribute(text, path.as_ref())?, "path='v.cgf'");
    assert!(resource.attachment.compared_group_id);
    let Some(PrefabAttributeObservation::Present(target)) = &resource.attachment.attachment_target
    else {
        return Err("expected the explicitly empty AttachmentTarget".into());
    };
    assert_eq!(attribute(text, Some(target))?, "AttachmentTarget=''");
    let PrefabModelSource::Brush(path) = &brush.model else {
        return Err("expected brush Prefab selector".into());
    };
    assert_eq!(attribute(text, path.as_ref())?, "Prefab='b.cgf'");
    assert_eq!(
        attribute(text, brush.transform.scale.as_ref())?,
        "Scale='2,2,2'"
    );
    assert!(brush.attachment.attachment_target.is_none());

    check_group_sources(text, resource, brush)
}

fn check_group_sources(
    text: &str,
    resource: &PrefabObjectSource,
    brush: &PrefabObjectSource,
) -> TestResult {
    let group = resource.groups.as_ref().ok_or("parent transform")?;
    assert!(
        brush
            .groups
            .as_ref()
            .is_some_and(|other| std::sync::Arc::ptr_eq(group, other)),
        "siblings reuse the actual immediate parent"
    );
    assert_eq!(
        attribute(text, group.id.as_ref())?,
        "Id='a'",
        "the comparison retained the parent Id"
    );
    assert_eq!(
        attribute(text, group.transform.position.as_ref())?,
        "Pos='1,2,3'",
        "the enclosing translation points to its original attribute"
    );
    assert!(
        group.transform.rotation.is_none(),
        "the parent rotation uses its node-scoped default"
    );
    Ok(())
}
