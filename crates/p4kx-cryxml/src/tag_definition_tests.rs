// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::tag_definition::{Member, Section, TagDefinition};
use crate::{SourceDocument, SourceReadOptions};

#[test]
fn authored_tag_sections_preserve_membership_order_and_exact_parse_ownership()
-> Result<(), Box<dyn std::error::Error>> {
    let bytes = br#"<TagDefinition version="2"><Tags>
       <Group name="Movement"><Tag name="One"/><Tag name="One" subTagDef="Animations/Other.xml"/></Group>
       <Tag name="Two"/>
       <foreign:Tag xmlns:foreign="urn:test" name="Ignored"/>
       </Tags><Imports><Import filename="Animations/Shared.xml"/></Imports></TagDefinition>"#;
    let first = SourceDocument::parse(bytes, SourceReadOptions::default())?;
    let second = SourceDocument::parse(bytes, SourceReadOptions::default())?;
    let definition = TagDefinition::from_root(first.root()).ok_or("original definition root")?;
    let sections: Vec<_> = definition.sections().collect();
    let [Section::Tags(tags), Section::Imports(imports)] = sections.as_slice() else {
        return Err("the original sections must preserve their order".into());
    };
    let members: Vec<_> = tags.members().collect();
    let [Member::Group(group), Member::Tag(single)] = members.as_slice() else {
        return Err("one group and one unqualified direct Tag".into());
    };
    let group: Vec<_> = group.tags().collect();
    let [before, after] = group.as_slice() else {
        return Err("duplicate tag spellings retain their physical occurrences".into());
    };
    assert_eq!(
        before.attribute("name"),
        after.attribute("name"),
        "the authored spelling repeats"
    );
    assert_ne!(
        before.position(),
        after.position(),
        "the repeated spelling has separate coordinates"
    );
    assert_eq!(
        single.attribute("name"),
        Some("Two"),
        "the second member stays at its source position"
    );
    assert!(
        first.owns_element(*before),
        "the actual source parse owns the element"
    );
    assert!(
        !second.owns_element(*before),
        "another parse cannot admit an equal-looking element"
    );
    assert_eq!(
        imports.elements().count(),
        1,
        "the separately authored import remains visible"
    );
    Ok(())
}
