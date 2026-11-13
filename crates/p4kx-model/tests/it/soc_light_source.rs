// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_cryxml::SourceElement;
use p4kx_model::{LightState, SocEntityFile, SocLightMode, SocSourceValue};
use p4kx_testkit::{CryXmlByteOrder, CryXmlSignature, TestChunkFile, TestElement};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn file_bytes(xml: impl Into<Vec<u8>>) -> Vec<u8> {
    TestChunkFile::crch().chunk(0x0004, 3, xml).bytes()
}

#[test]
fn original_modes_preserve_order_zero_false_and_malformed_fields() -> TestResult {
    let bytes = file_bytes(
        br#"<Entities><Entity EntityClass="SyntheticProp"><PropertiesDataCore>
        <EntityComponentLight lightType="UnknownFixtureType" useTemperature="0">
        <emergencyState intensity="7"/>
        <defaultState intensity="0" lightStyle="0"><color r="0"/><color b="1"/></defaultState>
        <auxiliaryState temperature="NaN"/>
        <defaultState intensity="bad"/>
        <cinematicState/>
        <projectorParams FOV="9" fixtureExtra="kept"/>
        </EntityComponentLight></PropertiesDataCore></Entity></Entities>"#
            .as_slice(),
    );
    let file = SocEntityFile::parse(&bytes)?;
    let chunk = file
        .entity_chunks()
        .next()
        .ok_or("original entity chunk")??;
    let entity = chunk.entities().next().ok_or("original entity")?;
    let [light] = entity
        .authored_lights()
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|_| "one original light")?;
    assert_eq!(
        light.light_type(),
        SocSourceValue::Present("UnknownFixtureType"),
        "unsupported authored type survives"
    );
    assert_eq!(
        light.use_temperature(),
        SocSourceValue::Present(false),
        "explicit false survives"
    );
    let modes: Vec<_> = light.modes().collect();
    let states: Vec<_> = modes
        .iter()
        .map(|mode| (mode.kind(), mode.intensity()))
        .collect();
    assert_eq!(
        states,
        [
            (LightState::Emergency, SocSourceValue::Present(7.0)),
            (LightState::Default, SocSourceValue::Present(0.0)),
            (LightState::Auxiliary, SocSourceValue::Absent),
            (LightState::Default, SocSourceValue::Invalid),
            (LightState::Cinematic, SocSourceValue::Absent),
        ],
        "physical order and missing, zero and invalid values"
    );
    verify_mode_details(&modes)?;
    assert!(
        light
            .element()
            .elements()
            .any(|node| node.attribute("fixtureExtra") == Some("kept")),
        "unclassified source fields remain accessible"
    );
    Ok(())
}

#[test]
fn original_groups_keep_empty_sections_relative_frames_and_duplicate_attributes() -> TestResult {
    let root = group_fixture();
    let bytes = file_bytes(root.to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little));
    let file = SocEntityFile::parse(&bytes)?;
    let chunk = file
        .entity_chunks()
        .next()
        .ok_or("original binary entity chunk")??;
    let entity = chunk.entities().next().ok_or("original binary entity")?;
    let [direct, nested] = entity
        .authored_light_groups()
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|_| "both original group scopes")?;
    assert_eq!(
        direct.baked_sections().count(),
        2,
        "empty and populated sections are distinct"
    );
    assert_eq!(
        nested.lights().count(),
        0,
        "empty group yields no synthetic light"
    );
    let baked = direct.lights().next().ok_or("original baked light")?;
    assert_eq!(
        baked.relative_frames().count(),
        2,
        "conflicting original frames remain visible"
    );
    let component = baked
        .components()
        .next()
        .ok_or("original baked component")?;
    assert_eq!(
        component.use_temperature(),
        SocSourceValue::Invalid,
        "repeated boolean is ambiguous"
    );
    assert_eq!(
        component.element().attributes().count(),
        2,
        "both raw attributes retained"
    );
    Ok(())
}

fn verify_mode_details(modes: &[SocLightMode<'_>]) -> TestResult {
    let zero = modes.get(1).ok_or("authored zero mode")?;
    assert_eq!(
        zero.light_style(),
        SocSourceValue::Present(0),
        "authored zero light style"
    );
    let colours: Vec<_> = zero.colours().map(SourceElement::position).collect();
    assert!(
        matches!(colours.as_slice(), [first, second] if first != second),
        "repeated colour elements remain positioned"
    );
    let auxiliary = modes.get(2).ok_or("authored auxiliary mode")?;
    assert_eq!(
        auxiliary.temperature(),
        SocSourceValue::Invalid,
        "nonfinite input stays invalid"
    );
    Ok(())
}

fn group_fixture() -> TestElement {
    let group = TestElement::new("EntityComponentLightGroup")
        .child(TestElement::new("BakedInLights"))
        .child(
            TestElement::new("BakedInLights").child(
                TestElement::new("Light")
                    .child(TestElement::new("RelativeXForm").attribute("translation", "1,2,3"))
                    .child(TestElement::new("RelativeXForm").attribute("translation", "4,5,6"))
                    .child(
                        TestElement::new("EntityComponentLight")
                            .attribute("useTemperature", "0")
                            .attribute("useTemperature", "1")
                            .child(TestElement::new("defaultState")),
                    ),
            ),
        );
    TestElement::new("Entities").child(TestElement::new("Entity").child(group).child(
        TestElement::new("PropertiesDataCore").child(TestElement::new("EntityComponentLightGroup")),
    ))
}
