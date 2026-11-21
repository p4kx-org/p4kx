// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{
    ArchiveFiles, ArchiveModels, Assembly, AssemblyOptions, EntityInterior, InteriorLightHelper,
    InteriorLightHelperError, InteriorOptions, InteriorPart, LoadoutResolver,
    SocTileLightAnchorError, ValueOrigin,
};
use p4kx_extraction::{Archive, DataForgeEntry};
use p4kx_gltf::GlbOptions;
use p4kx_model::{LightState, SocSourceValue};
use p4kx_testkit::{
    DataForgeBuilder, TestChunkFile, TestContainer, TestEntity, TestEntry, TestModel, TestNode,
    check_glb, entity_schema,
};

use crate::assembly_support::{TestResult, fixture};

const OWNER: &str = "TEST_LightOwner";
const VEHICLE: &str = "Scripts/Entities/Vehicles/Implementations/Xml/Fixture.xml";
const ROOT: &str = "ObjectContainers/Ships/Fixture/parent.socpak";
const CHILD: &str = "ObjectContainers/Ships/Fixture/nested.socpak";
const ROTATED_SCALED: [[f32; 4]; 3] = [
    [0.0, -3.0, 0.0, 10.0],
    [2.0, 0.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
];
const IDENTITY: [[f32; 4]; 3] = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
];

const PORTS: &str = r#"
<TileItemPortEntries>
  <ItemPort name="fixture[tip]" interactionOffset="4,8,0"/>
  <ItemPort name="duplicate" interactionOffset="1,2,3"/>
  <ItemPort name="duplicate" interactionOffset="4,5,6"/>
</TileItemPortEntries>"#;
const VEHICLE_XML: &str = r#"<Vehicle>
  <Parts>
    <Part name="fixturePort" class="ItemPort">
      <ItemPort>
        <Types><Type type="MultiLight"/></Types>
        <AttachmentPoints>
          <AttachmentPoint id="tip" name="tip" bone="unused"/>
          <AttachmentPoint name="possible"/>
          <AttachmentPoint name="possible" bone="different"/>
        </AttachmentPoints>
      </ItemPort>
    </Part>
    <Part name="decoy" class="ItemPort">
      <ItemPort>
        <Types><Type type="WeaponGun"/></Types>
        <AttachmentPoints><AttachmentPoint name="tip" bone="unused"/></AttachmentPoints>
      </ItemPort>
    </Part>
  </Parts>
  <Modifications><Modification name="Selected"><Elems>
    <Elem idRef="tip" name="bone" value="direct"/>
  </Elems></Modification></Modifications>
</Vehicle>"#;

fn entity_xml() -> String {
    let entity = |name: &str, pos: &str, port: &str, helper: &str, lights: &str| {
        format!(
            r#"<Entity Name="{name}" EntityClass="LightGroup" Pos="{pos}">
            <TileAttachmentParams ItemPortName="{port}"/>
            <EntityComponentLightGroup {helper}><BakedInLights>{lights}</BakedInLights></EntityComponentLightGroup>
        </Entity>"#
        )
    };
    let light = |name: &str, pos: &str| {
        format!(
            r#"<Light Name="{name}"><RelativeXForm rotation="1,0,0,0" translation="{pos}"/>
          <EntityComponentLight lightType="Omni"><defaultState intensity="0"/><emergencyState intensity="2"/></EntityComponentLight></Light>"#
        )
    };
    let lights = light("first", "1,0,0") + &light("second", "0,1,0");
    let first = entity("source group", "1,2,3", "fixture[tip]", "", &lights);
    let conflict = entity(
        "conflict",
        "7,8,9",
        "duplicate",
        r#"helper="direct""#,
        &light("third", "0,0,0"),
    );
    let unbound = entity(
        "unbound",
        "0,0,0",
        "missing[tip]",
        "",
        &light("fourth", "0,0,0"),
    );
    let empty = entity("empty", "0,0,0", "duplicate", "", "");
    format!("<Entities>{first}{conflict}{unbound}{empty}</Entities>")
}

fn container(stem: &str, nested: Option<&str>) -> Vec<u8> {
    let nested = nested.map_or_else(String::new, |child| format!(
        r#"<ChildObjectContainers><Child name="{child}" pos="0,5,0" rot="1,0,0,0"/></ChildObjectContainers>"#
    ));
    let descriptor = format!(
        r#"<ObjectContainer><StreamingOCs><OC name="{stem}.soc"/></StreamingOCs>{nested}{PORTS}</ObjectContainer>"#
    );
    let data = TestChunkFile::crch().chunk(4, 0, entity_xml()).bytes();
    TestContainer::new()
        .entry(TestEntry::stored(format!("{stem}.xml"), descriptor))
        .entry(TestEntry::stored(format!("{stem}.soc"), data))
        .build()
}

fn entries() -> Vec<TestEntry> {
    let owner = TestEntity::new(1, OWNER, "entities/spaceships/fixture_owner.xml")
        .vehicle(VEHICLE, "Selected")
        .geometry("Objects/Test/Fixture.cgf", None)
        .object_container(ROOT, "room", [0.0; 3], [0.0; 3])
        .object_container(ROOT, "missing", [0.0; 3], [0.0; 3]);
    let database = entity_schema(DataForgeBuilder::new()).record(owner.record());
    let node = |name: &str, parent, local, kind| TestNode {
        name: name.to_owned(),
        parent,
        local,
        kind,
    };
    let model = TestModel::cube().with_nodes(vec![
        node("root", None, IDENTITY, 2),
        node("mesh", Some(0), IDENTITY, 0),
        node("room", Some(0), ROTATED_SCALED, 2),
        node("direct", Some(0), IDENTITY, 2),
        node("different", Some(0), IDENTITY, 2),
    ]);
    vec![
        TestEntry::stored("Data/Game2.dcb", database.build()),
        TestEntry::stored("Data/Objects/Test/Fixture.cgf", model.cgf()),
        TestEntry::stored("Data/Objects/Test/Fixture.cgfm", model.cgfm()),
        TestEntry::stored(format!("Data/{VEHICLE}"), VEHICLE_XML),
        TestEntry::stored(format!("Data/{ROOT}"), container("parent", Some(CHILD))),
        TestEntry::stored(format!("Data/{CHILD}"), container("nested", None)),
    ]
}

struct Built {
    assembly: Assembly,
    interior: EntityInterior,
}

fn build() -> Result<Built, Box<dyn std::error::Error>> {
    let fixture = fixture(entries())?;
    let archive = Archive::open(&fixture.path)?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let files = ArchiveFiles::new(&archive);
    let mut resolver = LoadoutResolver::new(&forge).with_files(&files);
    let record = resolver.entities().find(OWNER)?;
    let tree = resolver.resolve(&record)?;
    let vehicle = tree.vehicle().ok_or("owner vehicle declaration")?;
    let ports = vehicle.light_ports().collect::<Vec<_>>();
    let [port] = ports.as_slice() else {
        return Err("one original MultiLight port".into());
    };
    let [tip, first_duplicate, second_duplicate] = port.attachments() else {
        return Err("three original point rows".into());
    };
    assert_eq!(port.part().name(), "fixturePort", "decoy type excluded");
    assert_eq!(
        tip.bone(),
        SocSourceValue::Present("direct"),
        "selected bone"
    );
    assert_eq!(
        tip.bone_values().next(),
        Some(("direct", ValueOrigin::Elem)),
        "applied element origin"
    );
    assert_eq!(
        first_duplicate.name(),
        second_duplicate.name(),
        "duplicate name retained"
    );
    assert_eq!(
        first_duplicate.bone(),
        SocSourceValue::Absent,
        "missing bone retained"
    );
    let models = ArchiveModels::new(&archive);
    let assembly = Assembly::build(&tree, &models, &AssemblyOptions::default())?;
    let interior = EntityInterior::build(&archive, &assembly, &tree, &InteriorOptions::default())?;
    Ok(Built { assembly, interior })
}

fn close(a: [f32; 3], b: [f32; 3]) -> bool {
    a.into_iter().zip(b).all(|(a, b)| (a - b).abs() < 0.0001)
}

#[test]
fn direct_vehicle_light_group_keeps_original_offsets_and_exact_helper() -> TestResult {
    let built = build()?;
    let [placed, unproven] = built.interior.containers() else {
        return Err("one container reused at two placements".into());
    };
    let lights = placed
        .interior()
        .parts()
        .iter()
        .filter(|part| part.light().is_some())
        .collect::<Vec<_>>();
    let [first, second, conflict, unbound, nested_first, _, _, _] = lights.as_slice() else {
        return Err(format!(
            "expected four direct and four nested lights, got {}",
            lights.len()
        )
        .into());
    };
    let first_light = first.light().ok_or("first original light")?;
    assert_eq!(
        first_light.state,
        LightState::Emergency,
        "legacy active state"
    );
    assert!(
        close(first_light.transform.translation(), [2.0, 2.0, 3.0]),
        "original light frame"
    );
    assert!(
        close(first.local().translation(), [5.0, 2.0, 0.0]),
        "tile offset in container space"
    );
    assert!(
        close(second.local().translation(), [4.0, 3.0, 0.0]),
        "rigid group delta"
    );
    let evidence = first.light_placement().ok_or("direct evidence")?;
    let port = evidence.port.as_ref().ok_or("physical descriptor port")?;
    assert_eq!(
        port.descriptor.as_ref(),
        "parent.xml",
        "exact owning descriptor"
    );
    let InteriorLightHelper::Matched(helper) = &evidence.helper else {
        return Err("exact root model helper".into());
    };
    assert_eq!(helper.name, "direct", "unique effective model node");
    assert!(
        close(helper.helper_from_light.translation(), [4.0, 10.0, 0.0]),
        "helper frame reconstruction"
    );
    assert!(matches!(
        unbound.light_placement().map(|evidence| &evidence.helper),
        Some(InteriorLightHelper::Unresolved(
            InteriorLightHelperError::TilePortUnavailable
        )),
    ));
    verify_unresolved(conflict, nested_first, unproven)
}

fn verify_unresolved(
    conflict: &InteriorPart,
    nested_first: &InteriorPart,
    unproven: &p4kx_assembly::EntityContainer,
) -> TestResult {
    assert!(
        matches!(
            conflict
                .light_placement()
                .and_then(|evidence| evidence.anchor),
            Some(Err(SocTileLightAnchorError::AmbiguousSelector))
        ),
        "duplicate descriptor ports remain unresolved"
    );
    assert!(
        nested_first.light_placement().is_none(),
        "a nested occurrence does not borrow the root owner"
    );
    let unproven = unproven
        .interior()
        .parts()
        .iter()
        .find(|part| part.name() == "first")
        .ok_or("missing-helper fallback")?;
    assert!(
        matches!(
            unproven
                .light_placement()
                .and_then(|evidence| evidence.anchor),
            Some(Err(SocTileLightAnchorError::MissingContainerFrame))
        ),
        "fallback container transform is unsuitable for anchoring"
    );
    assert!(
        matches!(
            unproven.light_placement().map(|evidence| &evidence.helper),
            Some(InteriorLightHelper::Unresolved(
                InteriorLightHelperError::InvalidFrame
            ))
        ),
        "unproven owner cannot bind helper local frame"
    );
    Ok(())
}

#[test]
fn entity_glb_keeps_each_supported_light_once_in_its_own_container() -> TestResult {
    let built = build()?;
    let assembly = built.assembly.with_interior(built.interior);
    let mut bytes = Vec::new();
    assembly.write_glb(&GlbOptions::default(), &mut bytes)?;
    let glb = check_glb(&bytes)?;
    let count = glb
        .document
        .nodes()
        .filter(|node| node.light().is_some())
        .count();
    assert_eq!(
        count, 16,
        "eight lights per direct-and-nested pair across two owner placements"
    );
    Ok(())
}
