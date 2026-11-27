// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;
use std::io;
use std::ops::Range;
use std::sync::atomic::{AtomicUsize, Ordering};

use p4kx_dataforge::{
    Annotations, DataForge, LocalizationInput, ObservedResolution, PositionedSources,
    PositionedValue, RecordJsonBytes, RecordJsonSource, RecordXml, RecordXmlObserver,
    RecordXmlSite, RecordXmlSpan, RecordXmlToken, RecordXmlValueMember, Resolution, TextResolver,
    TreeBuilder, Value,
};
use p4kx_testkit::{DataForgeBuilder, TestStruct, TestType, TestValue};

use super::{TestResult, one_record, record};

const RESOLVED: &str = "Crate & \"Co\"\n\u{e9}";
const RAW: &str = "original unformatted value";
const ESCAPED: &[u8] = "Crate &amp; &quot;Co&quot;&#10;\u{e9}".as_bytes();

#[derive(Default)]
struct Texts {
    lookups: AtomicUsize,
}

impl TextResolver for Texts {
    fn resolve<'a>(&'a self, key: &str) -> Resolution<'a> {
        match key {
            "@found" | "@legacy" => Resolution::Text(Cow::Borrowed(RESOLVED)),
            "@emptyOutput" => Resolution::Text(Cow::Borrowed("")),
            "@placeholder" => Resolution::Keep,
            _ => Resolution::Missing,
        }
    }

    fn resolve_observed<'a>(&'a self, key: &str) -> ObservedResolution<'a> {
        self.lookups.fetch_add(1, Ordering::Relaxed);
        let resolution = self.resolve(key);
        match key {
            "@found" | "@emptyOutput" => ObservedResolution::from_lookup(resolution, Some(RAW)),
            "@blank" => ObservedResolution::from_lookup(resolution, Some("")),
            "@legacy" => ObservedResolution::unobserved(resolution),
            _ => ObservedResolution::from_lookup(resolution, None),
        }
    }
}

fn database() -> DataForgeBuilder {
    one_record(
        TestStruct::new("Item")
            .field("label", TestType::Locale)
            .field("absent", TestType::Locale)
            .field("blank", TestType::Locale)
            .field("emptyOutput", TestType::Locale)
            .field("legacy", TestType::Locale)
            .field("placeholder", TestType::Locale)
            .field("unset", TestType::Locale)
            .field("literal", TestType::String)
            .array("labels", TestType::Locale),
        record("Item")
            .value("label", TestValue::text("@found"))
            .value("absent", TestValue::text("@absent"))
            .value("blank", TestValue::text("@blank"))
            .value("emptyOutput", TestValue::text("@emptyOutput"))
            .value("legacy", TestValue::text("@legacy"))
            .value("placeholder", TestValue::text("@placeholder"))
            .value("unset", TestValue::text(""))
            .value("literal", TestValue::text("@found"))
            .value(
                "labels",
                TestValue::Array(vec![TestValue::text("@found"), TestValue::text("@absent")]),
            ),
    )
}

#[derive(Debug, PartialEq, Eq)]
enum Lookup {
    Unobserved,
    Lookup(Option<String>),
}

struct Locale {
    bytes: Range<usize>,
    key: String,
    lookup: Lookup,
    array_header: bool,
}

struct Observer<'a> {
    input: &'a [u8],
    locales: Vec<Locale>,
}

impl<'record, 'source> RecordXmlObserver<'record, 'source> for Observer<'_> {
    fn field(
        &mut self,
        _span: RecordXmlSpan<'record, 'source>,
        _slot: Option<PositionedValue<'record, 'source>>,
        _ancestors: &[PositionedSources],
    ) -> io::Result<()> {
        Ok(())
    }

    fn localization(
        &mut self,
        span: RecordXmlSpan<'record, 'source>,
        slot: Option<PositionedValue<'record, 'source>>,
        ancestors: &[PositionedSources],
        observed: &ObservedResolution<'_>,
    ) -> io::Result<()> {
        let slot = slot.ok_or_else(|| io::Error::other("no original locale slot"))?;
        let RecordXmlSite::Value {
            value: Value::Locale(key),
            member: RecordXmlValueMember::Translation,
        } = span.site
        else {
            return Err(io::Error::other("not an emitted translation"));
        };
        assert_eq!(
            span.token,
            RecordXmlToken::AttributeValue,
            "localized token"
        );
        assert!(
            matches!(slot.value(), Value::Locale(original) if original == key),
            "the address retains the original authored key"
        );
        self.check_source(slot, key, ancestors)?;
        let lookup = match observed.input() {
            LocalizationInput::Lookup(raw) => Lookup::Lookup(raw.map(str::to_owned)),
            _ => Lookup::Unobserved,
        };
        self.locales.push(Locale {
            bytes: range(span.bytes)?,
            key: (*key).to_owned(),
            lookup,
            array_header: !ancestors.is_empty(),
        });
        Ok(())
    }
}

impl Observer<'_> {
    fn check_source(
        &self,
        slot: PositionedValue<'_, '_>,
        key: &str,
        ancestors: &[PositionedSources],
    ) -> io::Result<()> {
        let local = slot.value_sources();
        assert!(local.complete(), "the key's original selection is complete");
        assert!(
            ancestors.iter().all(PositionedSources::complete),
            "container selectors are complete"
        );
        let mut kinds = Vec::new();
        for source in local.sources() {
            if let RecordJsonSource::Bytes { start, end, kind } = *source {
                kinds.push(kind);
                if kind == RecordJsonBytes::Text {
                    assert_eq!(
                        self.input.get(range(start..end)?),
                        Some(key.as_bytes()),
                        "original unescaped key bytes"
                    );
                }
            }
        }
        assert!(
            kinds.contains(&RecordJsonBytes::Text),
            "physical key text range"
        );
        if ancestors.is_empty() {
            assert!(
                kinds.contains(&RecordJsonBytes::Slot),
                "physical scalar key slot"
            );
        } else {
            assert!(
                kinds.contains(&RecordJsonBytes::ArrayElement),
                "physical element key slot"
            );
            assert!(
                ancestors
                    .iter()
                    .flat_map(PositionedSources::sources)
                    .any(|source| matches!(
                        source,
                        RecordJsonSource::Bytes {
                            kind: RecordJsonBytes::ArrayHeader,
                            ..
                        }
                    )),
                "physical array selector"
            );
        }
        Ok(())
    }
}

fn range(bytes: Range<u64>) -> io::Result<Range<usize>> {
    let convert = |index| usize::try_from(index).map_err(io::Error::other);
    Ok(convert(bytes.start)?..convert(bytes.end)?)
}

fn assert_locales(out: &[u8], locales: &[Locale]) {
    let found = || Lookup::Lookup(Some(RAW.to_owned()));
    let expected = [
        ("@found", ESCAPED, found()),
        ("@absent", b"true", Lookup::Lookup(None)),
        ("@blank", b"true", Lookup::Lookup(Some(String::new()))),
        ("@emptyOutput", b"", found()),
        ("@legacy", ESCAPED, Lookup::Unobserved),
        ("@found", ESCAPED, found()),
        ("@absent", b"true", Lookup::Lookup(None)),
    ];
    assert_eq!(
        locales.len(),
        expected.len(),
        "actual emitted localizations"
    );
    for (index, (locale, (key, escaped, lookup))) in locales.iter().zip(expected).enumerate() {
        assert_eq!(locale.key, key, "occurrence at {index}");
        assert_eq!(locale.lookup, lookup, "actual raw winner for {key}");
        assert_eq!(
            out.get(locale.bytes.clone()),
            Some(escaped),
            "escaped span for {key}"
        );
        assert_eq!(
            locale.array_header,
            index >= 5,
            "original container for {key}"
        );
    }
    assert!(
        locales
            .windows(2)
            .all(|pair| matches!(pair, [left, right] if left.bytes.end < right.bytes.start)),
        "repeated values retain distinct output ranges"
    );
}

#[test]
fn annotated_xml_observes_exact_output_ranges_and_original_locale_selection() -> TestResult {
    let input = database().build();
    let forge = DataForge::open(&input)?;
    let record = forge.records().next().ok_or("test Record")?;
    let record = TreeBuilder::default().build_positioned_partial(&record)?;
    let mut writer = RecordXml::new();
    for wrapped in [false, true] {
        let mut observer = Observer {
            input: &input,
            locales: Vec::new(),
        };
        let texts = Texts::default();
        let annotations = Annotations::none().with_texts(Some(&texts));
        let mut observed_xml = Vec::new();
        let mut ordinary_xml = Vec::new();
        if wrapped {
            writer.write_wrapped_positioned_observed_annotated(
                &record,
                annotations,
                1,
                &mut observed_xml,
                &mut observer,
            )?;
            assert_eq!(
                texts.lookups.load(Ordering::Relaxed),
                8,
                "one lookup per eligible occurrence"
            );
            writer.write_wrapped_element_annotated(
                record.tree(),
                annotations,
                1,
                &mut ordinary_xml,
            )?;
        } else {
            writer.write_positioned_observed_annotated(
                &record,
                annotations,
                &mut observed_xml,
                &mut observer,
            )?;
            assert_eq!(
                texts.lookups.load(Ordering::Relaxed),
                8,
                "one lookup per eligible occurrence"
            );
            writer.write_annotated(record.tree(), annotations, &mut ordinary_xml)?;
        }
        assert_eq!(
            observed_xml, ordinary_xml,
            "identical XML for the selected shape"
        );
        assert_locales(&observed_xml, &observer.locales);
    }
    Ok(())
}
