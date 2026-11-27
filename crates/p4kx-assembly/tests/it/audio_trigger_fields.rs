// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Trigger discovery by field name: `GlobalResourceAudio.audioTrigger` alone, or every string
//! field whose name contains `audioTrigger`.

use std::error::Error;

use p4kx_assembly::Entities;
use p4kx_assembly::audio::{
    PositionedTriggerReference, TriggerOccurrence, TriggerScope, positioned_trigger_references_in,
    trigger_references, trigger_references_in,
};
use p4kx_dataforge::{
    DataForge, PositionedRecord, PositionedValue, RecordJsonBytes, RecordJsonSource,
    RecordJsonUseRole, TreeBuilder,
};
use p4kx_testkit::{
    DataForgeBuilder, TestEntity, TestInstance, TestValue, audio_schema, entity_schema,
};

type TestResult = Result<(), Box<dyn Error>>;

fn melee() -> TestEntity {
    TestEntity::new(0x1, "TEST_Melee", "entities/test_melee.xml")
        .audio_trigger("Play_Slash")
        .component(TestValue::Struct(
            TestInstance::new("SMeleeAudioTestParams")
                .value("audioTriggerName", TestValue::text("Play_Swing"))
                .value("soundTag", TestValue::text("Play_Decoy")),
        ))
}

fn names(
    forge: &DataForge<'_>,
    scope: Option<TriggerScope>,
) -> Result<Vec<(String, String)>, Box<dyn Error>> {
    let tree = Entities::new(forge).find("TEST_Melee")?.tree()?;
    let references = scope.map_or_else(
        || trigger_references(&tree),
        |scope| trigger_references_in(&tree, scope),
    );
    Ok(references
        .iter()
        .map(|reference| (reference.name().to_owned(), reference.location().to_owned()))
        .collect())
}

#[test]
fn the_global_resource_audio_scope_ignores_other_structs() -> TestResult {
    let bytes = audio_schema(entity_schema(DataForgeBuilder::new()))
        .record(melee().record())
        .build();
    let forge = DataForge::open(&bytes)?;
    assert_eq!(
        names(&forge, Some(TriggerScope::GlobalResourceAudio))?,
        [(
            "Play_Slash".to_owned(),
            "Components[0].triggers[0].audioTrigger".to_owned()
        )]
    );
    Ok(())
}

#[test]
fn the_default_scope_adds_string_fields_named_like_audio_trigger() -> TestResult {
    let bytes = audio_schema(entity_schema(DataForgeBuilder::new()))
        .record(melee().record())
        .build();
    let forge = DataForge::open(&bytes)?;
    let expected = [
        (
            "Play_Slash".to_owned(),
            "Components[0].triggers[0].audioTrigger".to_owned(),
        ),
        (
            "Play_Swing".to_owned(),
            "Components[1].audioTriggerName".to_owned(),
        ),
    ];
    assert_eq!(names(&forge, None)?, expected, "default");
    assert_eq!(
        names(&forge, Some(TriggerScope::AudioTriggerFields))?,
        expected,
        "named scope; the soundTag field is no trigger"
    );
    Ok(())
}

#[test]
fn positioned_triggers_preserve_each_spelling_and_its_nested_source() -> TestResult {
    let bytes = audio_schema(entity_schema(DataForgeBuilder::new()))
        .record(
            melee()
                .audio_trigger("play_slash")
                .sound_effect("nested effect", "PLAY_SLASH")
                .record(),
        )
        .build();
    let forge = DataForge::open(&bytes)?;
    let entity = Entities::new(&forge).find("TEST_Melee")?;
    let positioned = TreeBuilder::new().build_positioned(&entity)?;
    for scope in [
        TriggerScope::AudioTriggerFields,
        TriggerScope::GlobalResourceAudio,
    ] {
        let ordinary = trigger_references_in(positioned.tree(), scope);
        let observed = positioned_trigger_references_in(&positioned, scope)?;
        assert_eq!(
            observed
                .iter()
                .map(PositionedTriggerReference::reference)
                .collect::<Vec<_>>(),
            ordinary.iter().collect::<Vec<_>>(),
            "the ordinary trigger lookup has the same rows and order"
        );
        let slash = observed.first().ok_or("the first trigger")?;
        assert_eq!(slash.reference().name(), "Play_Slash", "first spelling");
        assert_eq!(
            slash.reference().location(),
            "Components[0].triggers[0].audioTrigger",
            "first displayed location"
        );
        assert_eq!(slash.reference().count(), 3, "all three physical fields");
        let [first, second, nested] = slash.occurrences() else {
            return Err("one positioned occurrence per field".into());
        };
        let components = components(&positioned)?;
        assert_nested_sources(&bytes, components, [first, second, nested])?;
    }
    Ok(())
}

fn components<'record, 'source>(
    record: &'record PositionedRecord<'source>,
) -> Result<PositionedValue<'record, 'source>, Box<dyn Error>> {
    let position = record
        .tree()
        .fields()
        .iter()
        .position(|field| field.name() == "Components")
        .ok_or("the actual Components field")?;
    record
        .root_fields()?
        .get(position)
        .ok_or_else(|| "positioned Components array".into())
}

fn assert_nested_sources(
    bytes: &[u8],
    components: PositionedValue<'_, '_>,
    [first, second, nested]: [&TriggerOccurrence; 3],
) -> TestResult {
    let audio = components.element(0).ok_or("audio Component")?;
    let fields = audio.fields()?.ok_or("audio Component fields")?;
    let triggers = fields.get(0).ok_or("audio Component's triggers array")?;
    let effects = fields
        .get(1)
        .ok_or("audio Component's trigger effects array")?;
    let effect = effects.element(0).ok_or("nested sound effect")?;
    let original_audio = member(effect, 1)?;
    assert_occurrence(
        bytes,
        first,
        member(triggers.element(0).ok_or("first array element")?, 0)?,
        "Play_Slash",
    )?;
    assert_occurrence(
        bytes,
        second,
        member(triggers.element(1).ok_or("second array element")?, 0)?,
        "play_slash",
    )?;
    assert_occurrence(bytes, nested, member(original_audio, 0)?, "PLAY_SLASH")?;
    let outer = components
        .array_header_source()
        .ok_or("actual outer array")?;
    let direct = triggers
        .array_header_source()
        .ok_or("actual trigger array")?;
    let nested_array = effects
        .array_header_source()
        .ok_or("actual nested effect array")?;
    assert_ne!(direct, nested_array, "distinct physical array headers");
    for occurrence in [first, second, nested] {
        assert_origin(occurrence, outer, RecordJsonUseRole::Selector);
    }
    for occurrence in [first, second] {
        assert_origin(occurrence, direct, RecordJsonUseRole::Selector);
        assert!(
            !occurrence
                .sources()
                .iter()
                .any(|input| input.source == nested_array),
            "a direct trigger carries no selector from its sibling array"
        );
    }
    assert_origin(nested, nested_array, RecordJsonUseRole::Selector);
    assert_effect_pointer(effect, nested)
}

fn assert_effect_pointer(
    effect: PositionedValue<'_, '_>,
    nested: &TriggerOccurrence,
) -> TestResult {
    let pointer = effect
        .selection_sources()
        .and_then(|sources| {
            sources.sources().iter().copied().find(|source| {
                matches!(
                    source,
                    RecordJsonSource::Bytes {
                        kind: RecordJsonBytes::PointerSelector,
                        ..
                    }
                )
            })
        })
        .ok_or("actual nested effect pointer words")?;
    assert_origin(nested, pointer, RecordJsonUseRole::Selector);
    Ok(())
}

fn member<'record, 'source>(
    owned: PositionedValue<'record, 'source>,
    ordinal: usize,
) -> Result<PositionedValue<'record, 'source>, Box<dyn Error>> {
    owned
        .fields()?
        .and_then(|fields| fields.get(ordinal))
        .ok_or_else(|| format!("physical member {ordinal}").into())
}

fn assert_occurrence(
    bytes: &[u8],
    occurrence: &TriggerOccurrence,
    field: PositionedValue<'_, '_>,
    spelling: &str,
) -> TestResult {
    assert!(occurrence.complete(), "the complete original occurrence");
    let local = field.value_sources();
    for &source in local.sources() {
        let role = match source {
            RecordJsonSource::Bytes {
                start,
                end,
                kind: RecordJsonBytes::Text,
            } => {
                assert_eq!(
                    bytes.get(usize::try_from(start)?..usize::try_from(end)?),
                    Some(spelling.as_bytes()),
                    "the Payload bytes preserve this occurrence's own spelling"
                );
                RecordJsonUseRole::Payload
            }
            RecordJsonSource::PropertyStorage { .. } => RecordJsonUseRole::Conversion,
            _ => RecordJsonUseRole::Selector,
        };
        assert_origin(occurrence, source, role);
    }
    let names = field
        .field_name_sources()
        .ok_or("original property declaration")?;
    for &source in names.sources() {
        assert_origin(occurrence, source, RecordJsonUseRole::Selector);
    }
    assert_eq!(
        occurrence
            .sources()
            .iter()
            .filter(|input| input.role == RecordJsonUseRole::Payload)
            .count(),
        1,
        "only the exact trigger string is a payload"
    );
    Ok(())
}

fn assert_origin(
    occurrence: &TriggerOccurrence,
    coordinate: RecordJsonSource<'static>,
    role: RecordJsonUseRole,
) {
    assert!(
        occurrence
            .sources()
            .iter()
            .any(|input| input.source == coordinate && input.role == role),
        "{coordinate:?} is a {role:?} for the actual field"
    );
}
