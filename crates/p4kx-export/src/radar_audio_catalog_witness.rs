// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::Read as _;

use p4kx_extraction::cryxml::{
    SourceContent, SourceContentPosition, SourceDocument, SourceElement, SourceElementPosition,
    SourceReadOptions, SourceSyntax,
};
use p4kx_extraction::{
    Archive, ArchiveWatch, Checksum, FieldOrigin, SourceInventory, SourceLocation,
};
use p4kx_testkit::{real_archive_key, real_archive_path};

use super::TestResult;
use crate::catalog_queries::radar_audio::{
    AuthoredXmlElement, AuthoredXmlText, RadarAudioBindingStatus, RadarAudioDatum, RadarAudioField,
    RadarAudioNode, RadarAudioSelection, read_radar_audio,
};
use crate::catalog_v3::radar_audio_configuration::includes;

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn installed_radar_audio_reconciles_ordered_sound_slots_and_selector_coverage() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, real_archive_key().ok())?;
    let build = archive.build().ok_or("installed Build identity")?;
    let expected = std::env::var("P4KX_RADAR_AUDIO_BUILD_CHANGE")?;
    assert_eq!(build.change_number(), expected, "the selected Build");
    let watch = ArchiveWatch::new(&archive);
    watch.check()?;
    let mut entries = archive
        .entries()
        .iter()
        .filter(|entry| includes(entry.path()));
    let entry = entries.next().ok_or("physical radar-audio source")?;
    assert!(
        entries.next().is_none(),
        "installed sample has one physical owner"
    );
    assert!(
        matches!(entry.checksum(), Checksum::Crc32(_) | Checksum::Crc32c(_)),
        "installed sample has a declared CRC"
    );
    let mut bytes = Vec::new();
    let read = archive
        .open_entry_verified(entry)?
        .reject_overlong()
        .read_to_end(&mut bytes)?;
    assert_eq!(u64::try_from(read)?, entry.uncompressed_size());
    let original = SourceDocument::parse(&bytes, SourceReadOptions::default())?;
    assert!(
        !original.is_lossy(),
        "installed source has exact XML spelling"
    );
    assert!(
        original.root().name() == "RadarAudioData" && original.root().namespace().is_none(),
        "installed source is the unqualified radar root"
    );

    let snapshot = read_radar_audio(
        &archive,
        RadarAudioSelection::EntryIndex(u32::try_from(entry.index())?),
        None,
    )?;
    let [row] = snapshot.rows.as_slice() else {
        return Err("one projected installed source".into());
    };
    assert_eq!(row.identity().entry_index(), u32::try_from(entry.index())?);
    check_tree(row.elements(), original.root(), &snapshot.source_inventory)?;
    check_slots(row.elements(), original.root())?;
    watch.check()?;
    report(build.change_number());
    Ok(())
}

#[expect(
    clippy::print_stdout,
    reason = "installed witness prints only the verified Build and Entry count"
)]
fn report(change: &str) {
    println!("radar audio source witness: change={change}, entries=1");
}
fn check_tree(
    actual: &[RadarAudioNode],
    root: SourceElement<'_>,
    inventory: &SourceInventory,
) -> TestResult {
    let mut pending = vec![(root, None)];
    for (index, node) in actual.iter().enumerate() {
        let (expected, parent) = pending.pop().ok_or("original element occurrence")?;
        let syntax = expected.syntax()?;
        let authored = node.authored();
        assert!(
            authored.name().value().map(String::as_str) == Some(syntax.qualified_name()),
            "authored qualified element name survives verbatim"
        );
        assert_eq!(authored.parent().value(), Some(&parent));
        let position = element_position(expected.position())?;
        assert_eq!(authored.position().value(), Some(&position));
        check_use(authored.position().origin(), &position, inventory)?;
        check_declarations(authored, &syntax, inventory)?;
        check_text(authored, &syntax, inventory)?;

        let first_child = pending.len();
        let parent = Some(u32::try_from(index)?);
        pending.extend(expected.elements().map(|child| (child, parent)));
        pending
            .get_mut(first_child..)
            .ok_or("original child range")?
            .reverse();
    }
    assert!(
        pending.is_empty(),
        "every authored element occurrence is represented"
    );
    Ok(())
}

fn element_position(position: SourceElementPosition) -> TestResult<SourceLocation> {
    let (namespace, indices) = match position {
        SourceElementPosition::Text(range) => (
            "xmlSource.text",
            vec![u64::try_from(range.start)?, u64::try_from(range.end)?],
        ),
        SourceElementPosition::CryXml(index) => ("xmlSource.cryXml", vec![u64::from(index)]),
        _ => return Err("supported authored element coordinates".into()),
    };
    Ok(SourceLocation::Subresource {
        namespace: namespace.to_owned(),
        indices,
        field: "$node".to_owned(),
    })
}

fn check_declarations(
    authored: &AuthoredXmlElement,
    syntax: &SourceSyntax<'_>,
    inventory: &SourceInventory,
) -> TestResult {
    let declarations = syntax.declarations().collect::<Result<Vec<_>, _>>()?;
    assert_eq!(authored.declarations().len(), declarations.len());
    for (actual, expected) in authored.declarations().iter().zip(&declarations) {
        let name = expected.qualified_name();
        assert!(
            actual.name().value().map(String::as_str) == Some(name),
            "every authored declaration has its original qualified name and order"
        );
        let position = SourceLocation::xml_attribute(expected.position(), name)
            .ok_or("exact original declaration position")?;
        check_use(actual.name().origin(), &position, inventory)?;
        check_text_value(
            actual.value(),
            expected.value(),
            expected.lexeme(),
            &position,
            inventory,
        )?;
    }
    Ok(())
}

fn check_text(
    authored: &AuthoredXmlElement,
    syntax: &SourceSyntax<'_>,
    inventory: &SourceInventory,
) -> TestResult {
    let expected = if let Some(text) = syntax.binary_text() {
        vec![text]
    } else {
        let contents = syntax.content()?.collect::<Result<Vec<_>, _>>()?;
        contents
            .into_iter()
            .filter_map(|item| match item {
                SourceContent::Text(text) => Some(text),
                _ => None,
            })
            .collect()
    };
    assert_eq!(authored.content().len(), expected.len());
    for (actual, expected) in authored.content().iter().zip(&expected) {
        let (namespace, indices) = match expected.position() {
            SourceContentPosition::Text(range) => (
                "textXmlContent",
                vec![u64::try_from(range.start)?, u64::try_from(range.end)?],
            ),
            SourceContentPosition::CryXml(index) => ("cryxmlContent", vec![u64::from(*index)]),
            _ => return Err("supported original text coordinates".into()),
        };
        let position = SourceLocation::Subresource {
            namespace: namespace.to_owned(),
            indices,
            field: "$content".to_owned(),
        };
        check_text_value(
            actual,
            expected.value(),
            expected.lexeme(),
            &position,
            inventory,
        )?;
    }
    Ok(())
}

fn check_text_value(
    actual: &AuthoredXmlText,
    expected: &str,
    lexeme: Option<&str>,
    position: &SourceLocation,
    inventory: &SourceInventory,
) -> TestResult {
    assert!(
        actual.value().value().map(String::as_str) == Some(expected),
        "original decoded text survives verbatim"
    );
    assert!(
        actual
            .lexeme()
            .and_then(|cell| cell.value())
            .map(String::as_str)
            == lexeme.filter(|literal| *literal != expected),
        "distinct lexical spelling survives verbatim"
    );
    check_use(actual.value().origin(), position, inventory)?;
    if let Some(original) = actual.lexeme() {
        check_use(original.origin(), position, inventory)?;
    }
    Ok(())
}

fn check_use(
    origin: &FieldOrigin,
    position: &SourceLocation,
    inventory: &SourceInventory,
) -> TestResult {
    let [id] = origin.source_uses_refs() else {
        return Err("one direct source use per authored occurrence".into());
    };
    let use_row = inventory
        .iter_uses()
        .find(|usage| usage.id() == id)
        .ok_or("use in query inventory")?;
    assert!(
        use_row.location() == position,
        "source use retains the exact original coordinates"
    );
    Ok(())
}

fn check_slots(actual: &[RadarAudioNode], root: SourceElement<'_>) -> TestResult {
    let slots = root
        .elements()
        .filter(|node| node.namespace().is_none() && node.name() == "SoundEvents")
        .flat_map(SourceElement::elements)
        .filter(|node| node.namespace().is_none() && node.name() == "SoundEvent")
        .map(|node| {
            node.attributes()
                .find(|attr| attr.namespace().is_none() && attr.name() == "value")
                .map(|attr| attr.value())
        })
        .collect::<Vec<_>>();
    assert!(
        slots
            .iter()
            .any(|state| state.is_some_and(|value| !value.is_empty())),
        "installed witness includes a nonempty authored token",
    );
    let projected: Vec<_> = actual
        .iter()
        .filter_map(|node| node.binding_status().map(|binding| (node, binding)))
        .collect();
    assert_eq!(slots.len(), projected.len());
    for (raw, (node, binding)) in slots.into_iter().zip(projected) {
        let cell = node
            .fields()
            .iter()
            .find(|cell| cell.field() == RadarAudioField::SoundEventValue)
            .ok_or("sound-event slot")?;
        let actual = match cell.value().value() {
            Some(RadarAudioDatum::Empty) => Some(""),
            Some(RadarAudioDatum::Text { token }) => Some(token.as_str()),
            _ => None,
        };
        assert!(actual == raw, "authored slot token survives verbatim");
        let expected = match raw {
            None => RadarAudioBindingStatus::MissingValue,
            Some("") => RadarAudioBindingStatus::EmptyValue,
            Some(_) => RadarAudioBindingStatus::UnprovenSelector,
        };
        assert_eq!(binding.value(), Some(&expected));
    }
    Ok(())
}
