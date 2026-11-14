// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_testkit::{CryXmlByteOrder, CryXmlSignature, TestElement};

use crate::source_document::{SourceDocument, SourceReadOptions};
use crate::source_position::SourceAttributePosition;

use super::{AnimationDatabase, FragmentOption, LayerItem, attributes};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

#[test]
fn declared_reload_alternatives_keep_conditions_and_original_timing() -> TestResult {
    let parent = SourceDocument::parse(
        br#"<AnimDB FragDef="fragments.xml" TagDef="tags.xml"><Other/><SubADBs>
            <SubADB File="selected.adb" Tags="equipment">
                <Other/><FragmentID Name="Reload"/><FragmentID Name="ReloadEmpty"/>
            </SubADB><SubADB File="other.adb" Tags="different"><FragmentID Name="Reload"/></SubADB>
        </SubADBs></AnimDB>"#,
        SourceReadOptions::default(),
    )?;
    let parent = AnimationDatabase::from_root(parent.root()).ok_or("parent AnimDB")?;
    let branches = parent.branches().collect::<Vec<_>>();
    let [branch, other] = branches.as_slice() else {
        return Err("two independent authored alternatives".into());
    };
    assert_eq!(
        branch.ordinals(),
        (1, 0),
        "original section and branch sibling positions"
    );
    assert_eq!(other.ordinals(), (1, 1), "second branch position");
    assert_eq!(
        branch.files().next().ok_or("File occurrence")?.value(),
        "selected.adb",
        "original target selector"
    );
    assert_eq!(
        branch.tags().next().ok_or("Tags occurrence")?.value(),
        "equipment",
        "original branch condition"
    );
    assert_eq!(
        parent.fragment_definition().count(),
        1,
        "physical fragment definition selector"
    );
    assert_eq!(
        parent.tag_definition().count(),
        1,
        "physical global tag definition selector"
    );
    match_options(*branch)
}

fn match_options(branch: super::SubDatabase<'_>) -> TestResult {
    let fragments = branch.fragments().collect::<Vec<_>>();
    let [(first, named), (second, absent)] = fragments.as_slice() else {
        return Err("two authored FragmentID selectors".into());
    };
    assert_eq!(
        (*first, *second),
        (1, 2),
        "irrelevant siblings occupy original positions"
    );
    let target = options()?;
    let target = AnimationDatabase::from_root(target.root()).ok_or("target AnimDB")?;
    let name = attributes(*named, "Name").next().ok_or("authored Name")?;
    let aliases = target.options_named(name.value()).collect::<Vec<_>>();
    let missing = attributes(*absent, "Name")
        .next()
        .ok_or("authored missing Name")?;
    assert_eq!(
        target.options_named(missing.value()).count(),
        0,
        "no fabricated option"
    );
    let [main, next] = aliases.as_slice() else {
        return Err("two original options share this fragment name".into());
    };
    assert_eq!(
        main.ordinals(),
        (1, 1, 1),
        "unrelated nodes retain distinct physical ordinals"
    );
    assert_eq!(
        next.ordinals(),
        (1, 1, 2),
        "same-name alternative retains position"
    );
    assert_eq!(
        attributes(main.element(), "FragTags")
            .next()
            .ok_or("fragment condition")?
            .value(),
        "alternate",
        "fragment-local condition",
    );
    timing(*main)
}

#[test]
fn source_positions_distinguish_repeated_binary_timing_attributes() -> TestResult {
    let clip = TestElement::new("Animation")
        .attribute("name", "clip")
        .attribute("speed", "0")
        .attribute("speed", "1.25");
    let option = TestElement::new("Fragment").child(TestElement::new("AnimLayer").child(clip));
    let bytes = TestElement::new("AnimDB")
        .child(TestElement::new("FragmentList").child(TestElement::new("Reload").child(option)))
        .to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little);
    let original = SourceDocument::parse(&bytes, SourceReadOptions::default())?;
    let original = AnimationDatabase::from_root(original.root()).ok_or("binary AnimDB")?;
    let fragment = original
        .options_named("Reload")
        .next()
        .ok_or("original fragment")?;
    let layer = fragment.layers().next().ok_or("original animation layer")?;
    let Some((0, LayerItem::Animation(animation))) = layer.items().next() else {
        return Err("original animation child".into());
    };
    let speeds = animation.speeds().collect::<Vec<_>>();
    let [first, last] = speeds.as_slice() else {
        return Err("both physical timing occurrences".into());
    };
    assert_eq!(
        [first.value(), last.value()],
        ["0", "1.25"],
        "duplicate speed declarations remain ordered"
    );
    assert!(
        first.position() != last.position()
            && matches!(first.position(), SourceAttributePosition::CryXml { .. })
            && matches!(last.position(), SourceAttributePosition::CryXml { .. }),
        "both occurrences retain independent physical binary coordinates",
    );
    Ok(())
}

fn options() -> TestResult<SourceDocument<'static>> {
    Ok(SourceDocument::parse(
        br#"<AnimDB><Ignored/><FragmentList><A/><Reload><Ignored/>
             <Fragment BlendOutDuration="0.20" Tags="global" FragTags="alternate">
                 <ProcLayer/><AnimLayer><Ignored/><Blend Duration="0.33" StartTime="-1" ExitTime="0"/>
                     <Animation name="clip" speed="1.250" flags="loop"/><Animation name="other"/>
                 </AnimLayer>
             </Fragment><Fragment BlendOutDuration="0.5" Tags="different"/>
        </Reload></FragmentList></AnimDB>"#,
        SourceReadOptions::default(),
    )?)
}

fn timing(option: FragmentOption<'_>) -> TestResult {
    let blend_out = option
        .blend_out_durations()
        .next()
        .ok_or("authored blend-out")?;
    assert_eq!(
        blend_out.value(),
        "0.20",
        "unchanged original timing lexeme"
    );
    assert!(
        matches!(blend_out.position(), SourceAttributePosition::Text(_)),
        "physical source attribute coordinate"
    );
    let layer = option.layers().next().ok_or("authored animation layer")?;
    assert_eq!(
        layer.ordinal(),
        1,
        "preceding procedural layer retains its position"
    );
    let items = layer.items().collect::<Vec<_>>();
    let [
        (0, LayerItem::Other(_)),
        (1, LayerItem::Blend(blend)),
        (2, LayerItem::Animation(animation)),
        (3, LayerItem::Animation(unset)),
    ] = items.as_slice()
    else {
        return Err("interleaved source positions of blend and original alternatives".into());
    };
    for (actual, expected) in [
        (blend.durations().next(), "0.33"),
        (blend.start_times().next(), "-1"),
        (blend.exit_times().next(), "0"),
        (animation.speeds().next(), "1.250"),
    ] {
        let attribute = actual.ok_or("authored timeline value")?;
        assert_eq!(
            attribute.value(),
            expected,
            "original signs and lexical precision"
        );
        assert!(
            matches!(attribute.position(), SourceAttributePosition::Text(_)),
            "coordinate remains available to source evidence"
        );
    }
    assert!(
        unset.speeds().next().is_none(),
        "missing speed stays explicitly missing"
    );
    Ok(())
}
