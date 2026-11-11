// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Original title fixture source identities and observed field locations.

use p4kx_extraction::{
    DataForgeOrigin, Field, Language, LocalizationSet, RecordTree, SourceEncryption, StructValue,
    Value,
};

use crate::health_origin_fixture::{self, Fixture, Input, TestResult};

use super::{
    BROKER, BROKER_TITLE, CHILD, CHILD_PIECE, CONTRACT, DESCRIPTION, ENGLISH_DESCRIPTION,
    ENGLISH_TITLE, GENERATOR, GERMAN_DESCRIPTION, GERMAN_TITLE, LATER, NESTED, Original,
    PARENT_PIECE, REJECTED, Site, TEMPLATE, TEMPLATE_TITLE, TITLE,
};

pub(super) fn original(fixture: &Fixture, input: Input) -> TestResult<Original> {
    match fixture.entry.origin() {
        DataForgeOrigin::Archive(origin) => {
            assert!(
                !matches!(input, Input::Standalone),
                "standalone source has no Archive lineage"
            );
            let expected = if matches!(input, Input::Encrypted) {
                SourceEncryption::Encrypted
            } else {
                SourceEncryption::Unencrypted
            };
            assert_eq!(
                origin.encryption(),
                expected,
                "actual DCB payload encryption"
            );
        }
        DataForgeOrigin::File(_) => assert!(
            matches!(input, Input::Standalone),
            "only the standalone fixture has a file origin"
        ),
        _ => return Err("expected actual Archive Entry or standalone file".into()),
    }
    original_languages(fixture)?;
    let forge = fixture.entry.open()?;
    let generator = forge.record(0).ok_or("original Generator index")?;
    let template = forge.record(1).ok_or("original Template index")?;
    let broker = forge.record(2).ok_or("original Broker index")?;
    for (record, guid) in [
        (&generator, GENERATOR),
        (&template, TEMPLATE),
        (&broker, BROKER),
    ] {
        assert_eq!(
            record.guid().to_string(),
            health_origin_fixture::guid(guid),
            "the physical Record is the actual source control"
        );
    }
    let generator_tree = generator.tree()?;
    let template_tree = template.tree()?;
    let broker_tree = broker.tree()?;
    for (tree, record) in [
        (&generator_tree, &generator),
        (&template_tree, &template),
        (&broker_tree, &broker),
    ] {
        assert!(
            tree.is_from(record),
            "the original tree belongs to that exact Record"
        );
        assert_eq!(
            tree.error_count(),
            0,
            "every controlled source field is readable"
        );
    }
    sites(&generator_tree, &template_tree, &broker_tree)
}

fn original_languages(fixture: &Fixture) -> TestResult {
    let sources = LocalizationSet::read_all(&fixture.archive)?;
    let texts = sources.index()?;
    for (language, encryption) in [
        (Language::English, SourceEncryption::Encrypted),
        (Language::German, SourceEncryption::Unencrypted),
        (Language::French, SourceEncryption::Encrypted),
    ] {
        let entry = sources
            .entry(language)
            .ok_or("actual original language Entry")?;
        assert_eq!(
            entry.origin().ok_or("actual locale origin")?.encryption(),
            encryption,
            "the selected language Entry carries its fixture encryption"
        );
    }
    assert_eq!(
        texts.text(TITLE, Language::English),
        Some(ENGLISH_TITLE),
        "the actual preferred suffixed translation supplies the English tokens"
    );
    assert_eq!(
        texts.text(DESCRIPTION, Language::English),
        Some(ENGLISH_DESCRIPTION),
        "the original indexed text keeps padding and its authored escape"
    );
    assert_eq!(
        texts.text(TITLE, Language::German),
        Some(GERMAN_TITLE),
        "the actual German locale retains its literal title"
    );
    assert_eq!(
        texts.text(DESCRIPTION, Language::German),
        Some(GERMAN_DESCRIPTION),
        "the actual German locale retains its literal description"
    );
    assert_eq!(
        texts.text(TITLE, Language::French),
        None,
        "the actual French file is present and its title key is absent"
    );
    for (key, text) in [
        (PARENT_PIECE, "Parent ~mission(Nested)"),
        (CHILD_PIECE, "Child ~mission(Nested)"),
        (NESTED, "ready"),
        (LATER, "Later declaration"),
        (TEMPLATE_TITLE, "Template fallback"),
        (BROKER_TITLE, "Broker fallback"),
    ] {
        assert_eq!(
            texts.text(key, Language::English),
            Some(text),
            "the actual eligible and unused localized candidates are distinguishable"
        );
    }
    Ok(())
}

fn sites(
    generator: &RecordTree<'_>,
    template: &RecordTree<'_>,
    broker: &RecordTree<'_>,
) -> TestResult<Original> {
    let [handler] = array(generator.root(), "generators")? else {
        return Err("one actual original handler".into());
    };
    let [contract] = array(structure(handler)?, "contracts")? else {
        return Err("one actual contract slot".into());
    };
    let [child] = array(structure(contract)?, "subContracts")? else {
        return Err("one actual sub contract slot".into());
    };
    let parameters = structure(field(structure(contract)?, "paramOverrides")?)?;
    let child_parameters = structure(field(structure(child)?, "paramOverrides")?)?;
    assert_contract_links(structure(contract)?, structure(child)?, template, broker)?;
    let [title] = array(parameters, "stringParamOverrides")? else {
        return Err("one actual parent title".into());
    };
    let [rejected, later] = array(child_parameters, "stringParamOverrides")? else {
        return Err("two original ordered sub title selectors".into());
    };
    let [parent_piece, nested] = array(parameters, "propertyOverrides")? else {
        return Err("the exact parent binding order".into());
    };
    let [child_piece] = array(child_parameters, "propertyOverrides")? else {
        return Err("the actual nearest child binding".into());
    };
    let display = structure(field(template.root(), "contractDisplayInfo")?)?;
    let [template_title] = array(display, "displayString")? else {
        return Err("the actual first Template display slot".into());
    };
    Ok(Original {
        handler: site(generator, handler)?,
        contract: site(generator, contract)?,
        child: site(generator, child)?,
        title: site(generator, param(title, TITLE)?)?,
        description: site(broker, locale(broker.root(), "description", DESCRIPTION)?)?,
        parent_piece: site(generator, binding(parent_piece, "Piece", PARENT_PIECE)?)?,
        child_piece: site(generator, binding(child_piece, "pIECe", CHILD_PIECE)?)?,
        nested: site(generator, binding(nested, "Nested", NESTED)?)?,
        rejected: site(generator, param(rejected, REJECTED)?)?,
        later: site(generator, param(later, LATER)?)?,
        template_title: site(
            template,
            locale(structure(template_title)?, "value", TEMPLATE_TITLE)?,
        )?,
        broker_title: site(broker, locale(broker.root(), "title", BROKER_TITLE)?)?,
    })
}

fn assert_contract_links(
    contract: &StructValue<'_>,
    child: &StructValue<'_>,
    template: &RecordTree<'_>,
    broker: &RecordTree<'_>,
) -> TestResult {
    for (source, guid) in [(contract, CONTRACT), (child, CHILD)] {
        assert!(
            matches!(field(source, "id")?, Value::Guid(actual)
            if actual.to_string() == health_origin_fixture::guid(guid)),
            "the original logical identifier is stored at the selected occurrence"
        );
    }
    for (field_name, record_index) in [
        ("template", template.record_index()),
        ("missionBrokerEntry", broker.record_index()),
    ] {
        assert!(
            matches!(field(contract, field_name)?, Value::Reference(reference)
            if reference.record_index() == record_index && reference.location().is_none()),
            "the original relationship targets the actual dependency Record"
        );
    }
    Ok(())
}

fn param<'read, 'source>(
    value: &'read Value<'source>,
    key: &str,
) -> TestResult<&'read Value<'source>> {
    let owner = structure(value)?;
    assert_eq!(
        field(owner, "param")?,
        &Value::String("Title"),
        "both child occurrences really match the Title selector"
    );
    locale(owner, "value", key)
}

fn binding<'read, 'source>(
    value: &'read Value<'source>,
    token: &str,
    key: &str,
) -> TestResult<&'read Value<'source>> {
    let owner = structure(value)?;
    assert_eq!(
        field(owner, "extendedTextToken")?,
        &Value::String(token),
        "the actual token spelling is independent of its case-folded lookup"
    );
    let string_hash = structure(field(owner, "value")?)?;
    assert_eq!(
        string_hash.type_name(),
        "MissionPropertyValue_StringHash",
        "the original override uses the localized string-hash variant"
    );
    let [option] = array(string_hash, "options")? else {
        return Err("the one actual localized option".into());
    };
    locale(structure(option)?, "textId", key)
}

fn locale<'read, 'source>(
    parent: &'read StructValue<'source>,
    name: &str,
    key: &str,
) -> TestResult<&'read Value<'source>> {
    let value = field(parent, name)?;
    assert_eq!(
        value,
        &Value::Locale(key),
        "actual authored Locale at {name}"
    );
    Ok(value)
}

fn structure<'read, 'source>(
    value: &'read Value<'source>,
) -> TestResult<&'read StructValue<'source>> {
    let Value::Struct(value) = value else {
        return Err("actual source Struct".into());
    };
    Ok(value)
}

fn array<'read, 'source>(
    parent: &'read StructValue<'source>,
    name: &str,
) -> TestResult<&'read [Value<'source>]> {
    let Value::Array(values) = field(parent, name)? else {
        return Err(format!("actual source array {name}").into());
    };
    Ok(values)
}

fn field<'read, 'source>(
    parent: &'read StructValue<'source>,
    name: &str,
) -> TestResult<&'read Value<'source>> {
    parent
        .field(name)
        .ok_or_else(|| format!("actual source field {name}").into())
}

fn site<'source>(tree: &RecordTree<'source>, selected: &Value<'source>) -> TestResult<Site> {
    let mut pending: Vec<_> = tree
        .root()
        .fields()
        .iter()
        .rev()
        .map(Field::value)
        .collect();
    let mut ordinal = 0;
    while let Some(value) = pending.pop() {
        if std::ptr::eq(value, selected) {
            return Ok(Site {
                record: tree.record_index(),
                ordinal,
            });
        }
        ordinal += 1;
        match value {
            Value::Struct(structure) => {
                pending.extend(structure.fields().iter().rev().map(Field::value));
            }
            Value::Array(values) => pending.extend(values.iter().rev()),
            _ => {}
        }
    }
    Err("actual value absent from its original physical tree".into())
}
