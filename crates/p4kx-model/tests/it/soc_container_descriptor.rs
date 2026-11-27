// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_cryxml::SourceElementPosition;
use p4kx_model::{SocContainerDescriptor, SocSourceValue};
use p4kx_testkit::{CryXmlByteOrder, CryXmlSignature, TestElement};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const DESCRIPTOR: &[u8] = br#"<ObjectContainer>
        <Unknown/>
        <ChildObjectContainers>
            <Extra/><Child name=""/><Child Name="reused.socpak"/><Child name="reused.socpak"/>
        </ChildObjectContainers>
        <StreamingOCs>
            <Unknown/>
            <OC name="a.soc">
                <Unknown/><StreamingOCs><OC NAME="b.soc"/><OC/></StreamingOCs>
            </OC>
            <OC name="c.soc"/>
        </StreamingOCs>
        <ChildObjectContainers><Child/></ChildObjectContainers>
    </ObjectContainer>"#;

#[test]
fn soc_descriptor_preserves_original_children_and_nested_stream_order() -> TestResult {
    use SocSourceValue::{Absent, Present};

    let descriptor = SocContainerDescriptor::parse(DESCRIPTOR)?;
    let children: Vec<_> = descriptor
        .children()
        .map(|entry| (entry.ordinal(), entry.name()))
        .collect();
    assert_eq!(
        children,
        [
            ((1, 1), Present("")),
            ((1, 2), Present("reused.socpak")),
            ((1, 3), Present("reused.socpak")),
            ((3, 0), Absent),
        ],
        "every physical declaration and its actual name state"
    );
    let [a, b, unnamed, c] = descriptor
        .streams()
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|_| "four physical stream declarations")?;
    let a_position = a.declaration().element().position();
    assert!(
        matches!(a_position, SourceElementPosition::Text(_)),
        "positions within original text input"
    );
    assert_eq!(
        [
            (
                a.declaration().ordinal(),
                a.declaration().name(),
                a.parent_position()
            ),
            (
                b.declaration().ordinal(),
                b.declaration().name(),
                b.parent_position()
            ),
            (
                unnamed.declaration().ordinal(),
                unnamed.declaration().name(),
                unnamed.parent_position()
            ),
            (
                c.declaration().ordinal(),
                c.declaration().name(),
                c.parent_position()
            ),
        ],
        [
            ((2, 1), Present("a.soc"), None),
            ((1, 0), Present("b.soc"), Some(a_position.clone())),
            ((1, 1), Absent, Some(a_position)),
            ((2, 2), Present("c.soc"), None),
        ],
        "stream declarations retain original physical positions and parent scope"
    );
    Ok(())
}

#[test]
fn soc_descriptor_rejects_duplicate_name_interpretation_and_retains_raw_attributes() -> TestResult {
    let fixture = TestElement::new("ObjectContainer").child(
        TestElement::new("ChildObjectContainers").child(
            TestElement::new("Child")
                .attribute("Name", "first")
                .attribute("name", "second")
                .attribute("pos", "1,2,3"),
        ),
    );
    let bytes = fixture.to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little);
    let descriptor = SocContainerDescriptor::parse(&bytes)?;
    let child = descriptor.children().next().ok_or("one authored child")?;
    assert_eq!(
        child.name(),
        SocSourceValue::Invalid,
        "both case forms occupy the name field"
    );
    let attributes: Vec<_> = child
        .name_attributes()
        .map(|attribute| (attribute.name(), attribute.value()))
        .collect();
    assert_eq!(
        attributes,
        [("Name", "first"), ("name", "second")],
        "original names and values"
    );
    assert!(
        matches!(child.element().position(), SourceElementPosition::CryXml(_)),
        "binary node coordinate"
    );
    Ok(())
}
