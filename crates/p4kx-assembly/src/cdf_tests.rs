// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::{DeclaredModel, is_character_definition, read, read_drawing};

const THRUSTER: &str = r#"<CharacterDefinition>
  <Model File="Objects\t\nozzle_CHR.chr" LODRatio="40"/>
  <AttachmentList>
    <Attachment Type="CA_BONE" AName="Housing" BoneName="housing_attach" Binding="Objects\t\housing.cga" Flags="0"/>
    <Attachment Type="CA_SKIN" AName="Nozzle" Binding="Objects\t\nozzle.skin" Material="Objects\t\paint.mtl"/>
  </AttachmentList>
</CharacterDefinition>"#;

#[test]
fn the_first_skin_is_drawn_and_the_other_attachments_are_listed() {
    let character = read(THRUSTER.as_bytes()).expect("a character");
    assert_eq!(character.model, "Objects/t/nozzle.skin");
    assert_eq!(character.material.as_deref(), Some("Objects/t/paint.mtl"));
    assert_eq!(character.left_out.len(), 0);
    let [housing] = character.bones.as_slice() else {
        panic!("one bone attachment: {:?}", character.bones);
    };
    assert_eq!(housing.name, "Housing");
    assert_eq!(housing.bone, "housing_attach");
    assert_eq!(housing.binding, "Objects/t/housing.cga");
    assert_eq!(housing.position, [0.0; 3]);
    assert_eq!(housing.rotation, [1.0, 0.0, 0.0, 0.0]);
}

#[test]
fn a_bone_attachment_reads_its_offset_rotation_and_material() {
    let text = r#"<CharacterDefinition><AttachmentList>
            <Attachment Type="CA_SKIN" Binding="s.skin"/>
            <Attachment Type="CA_BONE" BoneName="b" Binding="x\Part.cga" Material="m.mtl"
              RelPosition="1,-2.5,3" RelRotation="0,0,1,0"/>
            </AttachmentList></CharacterDefinition>"#;
    let character = read(text.as_bytes()).expect("a character");
    let [part] = character.bones.as_slice() else {
        panic!("one bone attachment: {:?}", character.bones);
    };
    assert_eq!(part.name, "Part");
    assert_eq!(part.material.as_deref(), Some("m.mtl"));
    assert_eq!(part.position, [1.0, -2.5, 3.0]);
    assert_eq!(part.rotation, [0.0, 0.0, 1.0, 0.0]);
}

#[test]
fn a_bone_attachment_that_cannot_be_read_is_left_out() {
    let text = r#"<CharacterDefinition><AttachmentList>
            <Attachment Type="CA_SKIN" Binding="s.skin"/>
            <Attachment Type="CA_BONE" Binding="nobone.cga"/>
            <Attachment Type="CA_BONE" BoneName="b" Binding="badpos.cga" RelPosition="1,2"/>
            <Attachment Type="CA_BONE" BoneName="b" Binding="zero.cga" RelRotation="0,0,0,0"/>
            <Attachment Type="CA_BONE" BoneName="b" Binding="nested.cdf"/>
            <Attachment Type="CA_SKIN" Binding="second.skin"/>
            </AttachmentList></CharacterDefinition>"#;
    let character = read(text.as_bytes()).expect("a character");
    assert_eq!(character.bones.len(), 0);
    assert_eq!(
        character.left_out,
        [
            "nobone.cga",
            "badpos.cga",
            "zero.cga",
            "nested.cdf",
            "second.skin"
        ]
    );
}

#[test]
fn without_a_skin_the_first_attachment_is_drawn() {
    let text = r#"<CharacterDefinition><AttachmentList>
            <Attachment Type="CA_BONE" Binding="a.cga"/><Attachment Type="CA_BONE" Binding="b.cga"/>
            </AttachmentList></CharacterDefinition>"#;
    let character = read(text.as_bytes()).expect("a character");
    assert_eq!(character.model, "a.cga");
    assert_eq!(character.left_out, ["b.cga"]);
}

#[test]
fn without_attachments_the_model_file_is_drawn() {
    let text = r#"<CharacterDefinition><Model File="m.chr"/></CharacterDefinition>"#;
    let character = read(text.as_bytes()).expect("a character");
    assert_eq!(character.model, "m.chr");
    assert_eq!(character.left_out.len(), 0);
}

#[test]
fn a_document_that_names_nothing_or_nests_a_definition_is_refused() {
    assert!(read(b"<CharacterDefinition/>").is_err());
    assert!(read(b"<Other/>").is_err());
    let nested = r#"<CharacterDefinition><AttachmentList>
            <Attachment Type="CA_SKIN" Binding="again.CDF"/></AttachmentList></CharacterDefinition>"#;
    assert!(read(nested.as_bytes()).is_err());
    assert!(read(b"not xml").is_err());
}

#[test]
fn a_nested_model_file_does_not_compete_with_a_bound_attachment() -> Result<(), String> {
    let text = br#"<CharacterDefinition><Model File="nested.CDF"/><AttachmentList>
            <Attachment Type="CA_BONE" Binding="a.cga"/>
            </AttachmentList></CharacterDefinition>"#;
    let (character, body) = read_drawing(text, false, |_| {
        Err::<Option<DeclaredModel<()>>, _>("the body reader accepts direct model files only")
    })
    .map_err(|error| format!("{error:?}"))?;
    assert_eq!(
        character.model, "a.cga",
        "the bound fallback remains selected"
    );
    assert!(
        body.is_none(),
        "the nested definition did not supply geometry"
    );
    Ok(())
}

#[test]
fn the_extension_is_matched_ignoring_case() {
    assert!(is_character_definition("Objects/x/y.CDF"));
    assert!(!is_character_definition("Objects/x/y.cdfm"));
    assert!(!is_character_definition(".cdf"));
    assert!(!is_character_definition("cdf"));
}
