// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Reading prefab libraries.

use p4kx_cryxml::{SourceAttributePosition, SourceElementPosition};
use p4kx_model::{MAX_GROUP_DEPTH, PrefabError, PrefabLibrary, PrefabLibraryNameSource};
use p4kx_testkit::{TestPrefabLibrary, TestPrefabObject};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const IDENTITY: [f64; 4] = [1.0, 0.0, 0.0, 0.0];
const QUARTER_Z: [f64; 4] = [
    std::f64::consts::FRAC_1_SQRT_2,
    0.0,
    0.0,
    std::f64::consts::FRAC_1_SQRT_2,
];

fn near(actual: [f32; 3], expected: [f32; 3]) -> bool {
    actual
        .iter()
        .zip(expected)
        .all(|(a, e)| (a - e).abs() < 1e-5)
}

fn sample() -> TestPrefabLibrary {
    TestPrefabLibrary::new("test/rooms").prefab(
        "hall",
        vec![
            TestPrefabObject::brush("wall", "objects/test/wall.cgf", [1.0, 2.0, 3.0], IDENTITY),
            TestPrefabObject::geometry_entity("door", "objects/test/door.cga", [0.0; 3], QUARTER_Z),
            TestPrefabObject::entity(
                "lamp",
                "Lamp",
                "objects/test/lamp.cga",
                [4.0, 0.0, 0.0],
                IDENTITY,
            ),
            TestPrefabObject::marker("TagPoint", "spawn", [0.0, 5.0, 0.0]),
            TestPrefabObject::nested("sub", "test/rooms", "closet", [9.0, 0.0, 0.0], QUARTER_Z),
        ],
    )
}

#[test]
fn objects_are_read_with_their_model_class_and_nested_prefab() -> TestResult {
    let library = PrefabLibrary::parse(sample().xml().as_bytes())?;
    assert_eq!(library.name, "test/rooms");
    let hall = library.find("HALL").ok_or("no hall")?;
    assert_eq!(hall.library, "test/rooms");
    assert_eq!(hall.invalid, 0);
    let by_name = |name: &str| hall.objects.iter().find(|object| object.name == name);
    assert_eq!(
        by_name("wall").and_then(|o| o.model.as_deref()),
        Some("objects/test/wall.cgf")
    );
    assert_eq!(
        by_name("door").and_then(|o| o.model.as_deref()),
        Some("objects/test/door.cga")
    );
    let lamp = by_name("lamp").ok_or("no lamp")?;
    assert_eq!(lamp.model.as_deref(), Some("objects/test/lamp.cga"));
    assert_eq!(lamp.class, "Lamp");
    assert_eq!(by_name("spawn").ok_or("no spawn")?.model, None);
    let sub = by_name("sub")
        .and_then(|o| o.nested.as_ref())
        .ok_or("no ref")?;
    assert_eq!(sub.name, "test/rooms.closet");
    assert!(sub.guid.starts_with('{'));
    Ok(())
}

#[test]
fn cryxml_and_text_read_alike() -> TestResult {
    let text = PrefabLibrary::parse(sample().xml().as_bytes())?;
    let binary = PrefabLibrary::parse(&sample().cryxml())?;
    assert_eq!(text, binary);
    Ok(())
}

#[test]
fn a_group_turns_and_moves_its_members_and_is_left_out() -> TestResult {
    let prefab = TestPrefabLibrary::new("g").prefab(
        "p",
        vec![TestPrefabObject::group(
            "grp",
            [10.0, 0.0, 0.0],
            QUARTER_Z,
            vec![TestPrefabObject::brush(
                "child",
                "objects/test/c.cgf",
                [1.0, 0.0, 0.0],
                IDENTITY,
            )],
        )],
    );
    let library = PrefabLibrary::parse(prefab.xml().as_bytes())?;
    let objects = &library.find("p").ok_or("no prefab")?.objects;
    assert_eq!(objects.len(), 1);
    let child = objects.first().ok_or("no child")?;
    assert_eq!(child.name, "child");
    assert!(near(child.transform.translation(), [10.0, 1.0, 0.0]));
    assert!(!child.attached);
    Ok(())
}

#[test]
fn a_scale_is_applied_before_the_rotation() -> TestResult {
    let prefab = TestPrefabLibrary::new("s").prefab(
        "p",
        vec![
            TestPrefabObject::brush("big", "objects/test/b.cgf", [0.0; 3], QUARTER_Z)
                .scaled([2.0, 2.0, 2.0]),
        ],
    );
    let library = PrefabLibrary::parse(prefab.xml().as_bytes())?;
    let object = library
        .find("p")
        .and_then(|p| p.objects.first())
        .ok_or("no object")?;
    let rows = object.transform.rows;
    assert!((rows[1][0] - 2.0).abs() < 1e-5);
    assert!((rows[0][1] + 2.0).abs() < 1e-5);
    Ok(())
}

#[test]
fn objects_with_unusable_transforms_are_counted_with_their_members() -> TestResult {
    let prefab = TestPrefabLibrary::new("bad").prefab(
        "p",
        vec![
            TestPrefabObject::brush("zero", "a.cgf", [0.0; 3], IDENTITY).rotate_text("0,0,0,0"),
            TestPrefabObject::brush("text", "a.cgf", [0.0; 3], IDENTITY).rotate_text("x,y"),
            TestPrefabObject::group(
                "grp",
                [0.0; 3],
                IDENTITY,
                vec![TestPrefabObject::brush("ok", "a.cgf", [0.0; 3], IDENTITY)],
            )
            .rotate_text("0,0,0,0"),
            TestPrefabObject::brush("kept", "a.cgf", [0.0; 3], IDENTITY).without_transform(),
        ],
    );
    let library = PrefabLibrary::parse(prefab.xml().as_bytes())?;
    let prefab = library.find("p").ok_or("no prefab")?;
    assert_eq!(prefab.invalid, 4);
    assert_eq!(prefab.objects.len(), 1);
    assert!(prefab.objects.iter().all(|o| o.name == "kept"));
    assert!(prefab.objects.iter().all(|o| o.transform.is_identity()));
    Ok(())
}

#[test]
fn an_object_with_another_parent_than_its_group_is_attached() -> TestResult {
    let prefab = TestPrefabLibrary::new("a").prefab(
        "p",
        vec![
            TestPrefabObject::brush("free", "a.cgf", [0.0; 3], IDENTITY),
            TestPrefabObject::brush("hung", "a.cgf", [0.0; 3], IDENTITY)
                .attribute("Parent", "{OTHER}"),
            TestPrefabObject::brush("bone", "a.cgf", [0.0; 3], IDENTITY)
                .attribute("AttachmentTarget", "mnt_top"),
        ],
    );
    let library = PrefabLibrary::parse(prefab.xml().as_bytes())?;
    let flags: Vec<bool> = library
        .find("p")
        .ok_or("no prefab")?
        .objects
        .iter()
        .map(|o| o.attached)
        .collect();
    assert_eq!(flags, [false, true, true]);
    Ok(())
}

#[test]
fn groups_nested_beyond_the_limit_are_left_out() -> TestResult {
    let mut object = TestPrefabObject::brush("deep", "a.cgf", [0.0; 3], IDENTITY);
    for level in 0..MAX_GROUP_DEPTH + 2 {
        object = TestPrefabObject::group(&format!("g{level}"), [0.0; 3], IDENTITY, vec![object]);
    }
    let library = TestPrefabLibrary::new("d").prefab("p", vec![object]);
    let library = PrefabLibrary::parse(&library.cryxml())?;
    let prefab = library.find("p").ok_or("no prefab")?;
    assert_eq!(prefab.objects.len(), 0);
    assert!(prefab.invalid > 0);
    Ok(())
}

#[test]
fn a_root_named_after_the_library_is_read() -> TestResult {
    let text = "<ht_lights sourceLevel=\"x\"><Prefab Name=\"a.b\" Id=\"{1}\" Library=\"l\"><Objects/></Prefab></ht_lights>";
    let library = PrefabLibrary::parse(text.as_bytes())?;
    assert_eq!(library.name, "ht_lights");
    assert_eq!(library.prefabs.len(), 1);
    assert!(library.find("A.B").is_some());
    Ok(())
}

#[test]
fn a_file_that_is_no_library_is_refused() {
    assert!(matches!(
        PrefabLibrary::parse(b"<ObjectContainer/>"),
        Err(PrefabError::NotLibrary { .. })
    ));
    assert!(PrefabLibrary::parse(b"not xml at all").is_err());
}

#[test]
fn a_non_uniform_scale_under_a_turned_group_scales_before_it_turns() -> TestResult {
    let prefab = TestPrefabLibrary::new("s").prefab(
        "p",
        vec![TestPrefabObject::group(
            "grp",
            [0.0; 3],
            QUARTER_Z,
            vec![
                TestPrefabObject::brush("m", "a.cgf", [1.0, 1.0, 1.0], IDENTITY)
                    .scaled([1.0, 2.0, 3.0]),
            ],
        )],
    );
    let library = PrefabLibrary::parse(prefab.xml().as_bytes())?;
    let object = library
        .find("p")
        .and_then(|p| p.objects.first())
        .ok_or("no object")?;
    let expected = [[0.0, -2.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 3.0]];
    for (row, wanted) in object.transform.rows.iter().zip(expected) {
        assert!(row.iter().zip(wanted).all(|(a, e)| (a - e).abs() < 1e-5));
    }
    assert!(near(object.transform.translation(), [-1.0, 1.0, 1.0]));
    Ok(())
}

fn text_attribute<'a>(
    bytes: &'a [u8],
    position: &SourceAttributePosition,
) -> Result<&'a [u8], &'static str> {
    let SourceAttributePosition::Text(range) = position else {
        return Err("expected a text attribute");
    };
    bytes.get(range.clone()).ok_or("attribute range must fit")
}

#[test]
fn positioned_prefab_library_records_the_authored_fields_and_selected_objects() -> TestResult {
    let text = concat!(
        "\u{feff}<PrefabsLibrary xmlns:extra=\"urn:extra\" extra:Name=\"decoy\" ",
        "Name=\" démo &amp; hall \"><Prefab extra:Name=\"decoy\" Name=\" hall \" Id=\"\">",
        "<Objects><Object Type=\"Prefab\" /><Object Type=\"Brush\" Rotate=\"0,0,0,0\" /></Objects>",
        "<Objects><Object Name=\"ignored\" /></Objects></Prefab><Prefab/></PrefabsLibrary>",
    );
    let bytes = text.as_bytes();
    let positioned = PrefabLibrary::parse_positioned(bytes)?;
    assert_eq!(positioned.library, PrefabLibrary::parse(bytes)?);
    assert_eq!(positioned.library.name, "démo & hall");
    let source = positioned.source.ok_or("expected original positions")?;
    let PrefabLibraryNameSource::Attribute(name) = &source.name else {
        return Err("expected original Name attribute".into());
    };
    assert_eq!(
        text_attribute(bytes, name)?,
        "Name=\" démo &amp; hall \"".as_bytes()
    );
    let row = source
        .prefabs
        .first()
        .ok_or("expected first row positions")?;
    assert_eq!(
        text_attribute(bytes, row.name.as_ref().ok_or("Name")?)?,
        b"Name=\" hall \""
    );
    assert_eq!(
        text_attribute(bytes, row.id.as_ref().ok_or("Id")?)?,
        b"Id=\"\""
    );
    let Some(SourceElementPosition::Text(objects)) = &row.objects else {
        return Err("expected the selected Objects position".into());
    };
    let object_text = text.get(objects.clone()).ok_or("Objects range must fit")?;
    assert!(
        object_text.starts_with("<Objects><Object Type=\"Prefab\""),
        "{object_text}"
    );
    assert!(!object_text.contains("ignored"), "{object_text}");
    let hall = positioned
        .library
        .find("hall")
        .ok_or("expected parsed hall")?;
    assert_eq!(hall.objects.len(), 1);
    assert_eq!(hall.invalid, 1);
    assert!(hall.objects.iter().all(|object| object.nested.is_some()));
    let empty = source
        .prefabs
        .get(1)
        .ok_or("expected second row positions")?;
    assert_eq!(empty.name, None);
    assert_eq!(empty.id, None);
    assert_eq!(empty.objects, None);
    Ok(())
}

#[test]
fn positioned_prefab_library_selects_the_tag_when_the_root_name_is_blank() -> TestResult {
    let text = b"<Library Name=\"   \"><Prefab Name=\"p\"/></Library>";
    let positioned = PrefabLibrary::parse_positioned(text)?;
    let source = positioned.source.ok_or("expected original positions")?;
    assert_eq!(positioned.library.name, "Library");
    assert_eq!(source.root, SourceElementPosition::Text(0..text.len()));
    assert_eq!(
        source.name,
        PrefabLibraryNameSource::Element(SourceElementPosition::Text(0..text.len()))
    );
    Ok(())
}

#[test]
fn positioned_prefab_library_preserves_cryxml_table_indices() -> TestResult {
    let bytes = TestPrefabLibrary::new("test")
        .prefab("p", Vec::new())
        .cryxml();
    let positioned = PrefabLibrary::parse_positioned(&bytes)?;
    assert_eq!(positioned.library, PrefabLibrary::parse(&bytes)?);
    let source = positioned.source.ok_or("expected original positions")?;
    assert_eq!(
        source.name,
        PrefabLibraryNameSource::Attribute(SourceAttributePosition::CryXml {
            node_index: 0,
            attribute_index: 0,
        })
    );
    let prefab = source
        .prefabs
        .first()
        .ok_or("expected first row positions")?;
    assert_eq!(prefab.element, SourceElementPosition::CryXml(1));
    assert_eq!(prefab.objects, Some(SourceElementPosition::CryXml(2)));
    assert_eq!(
        prefab.id,
        Some(SourceAttributePosition::CryXml {
            node_index: 1,
            attribute_index: 3,
        })
    );
    Ok(())
}

#[test]
fn positioned_prefab_library_omits_coordinates_when_text_is_repaired() -> TestResult {
    let text = b"<PrefabsLibrary Name=\"A\xFFB\"><Prefab Name=\"p\"/></PrefabsLibrary>";
    let positioned = PrefabLibrary::parse_positioned(text)?;
    assert_eq!(positioned.library, PrefabLibrary::parse(text)?);
    assert_eq!(positioned.library.name, "A\u{fffd}B");
    assert!(positioned.source.is_none());
    Ok(())
}

#[test]
fn positioned_prefab_library_omits_coordinates_when_cryxml_strings_are_repaired() -> TestResult {
    let mut bytes = TestPrefabLibrary::new("broken-library")
        .prefab("p", Vec::new())
        .cryxml();
    let marker = b"broken-library";
    let position = bytes
        .windows(marker.len())
        .position(|bytes| bytes == marker)
        .ok_or("expected original library name")?;
    *bytes
        .get_mut(position)
        .ok_or("expected library name byte")? = 0xff;

    let positioned = PrefabLibrary::parse_positioned(&bytes)?;
    assert_eq!(positioned.library, PrefabLibrary::parse(&bytes)?);
    assert!(positioned.source.is_none());
    Ok(())
}
