// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordCoverage, CatalogRecordReference,
    CatalogRecordSelection, DatasetAvailability, MineableBaseSignature, MineableEntity,
    MineableSignature, ProvenanceMode,
};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForgeEntry, DataForgeOrigin, InputScope, RecordTree,
    SourceEncryption, SourceInput, SourceScope, SourceUses, SourcedValue, StructValue,
    Value as Original,
};
use p4kx_testkit::{real_archive_key, real_archive_path};

use super::TestResult;
use crate::real_archive_item_durability::{self as proof, Trace};
use crate::real_archive_verification::print_line;

#[path = "mineable_entity_sdk_sample.rs"]
mod sample;

use self::sample::{MINING, RADAR, SignatureLayout};

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and run this test with --ignored"]
fn installed_mineable_entity_sdk_preserves_original_links_and_both_scan_sources() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, real_archive_key().ok())
        .map_err(|_| "open the installed Archive")?;
    let build = archive
        .build()
        .ok_or("the installed Archive needs a Build")?;
    let identity = [
        build.branch(),
        build.version(),
        build.change_number(),
        build.build_id(),
    ];
    assert!(
        identity.iter().all(|part| !part.trim().is_empty()),
        "the installed Mineable witness uses the exact four-part Build identity"
    );
    let watch = ArchiveWatch::new(&archive);
    watch
        .check()
        .map_err(|_| "validate Archive before decoding")?;
    let entry = DataForgeEntry::read(&archive).map_err(|_| "decode the original DataForge")?;
    let forge = entry.open().map_err(|_| "open the original DataForge")?;
    let global = sample::layout(&forge, &watch)?;
    let owner = sample::select_owner(&forge, &watch, &global)?;
    let DataForgeOrigin::Archive(original) = entry.origin() else {
        return Err("an Archive DataForge Entry".into());
    };
    let scope = SourceUses::with_compact_coordinates()
        .observe(SourceInput::Archive(original), InputScope::Payload)?;
    let encrypted = match entry.origin().encryption() {
        SourceEncryption::Encrypted => Some(true),
        SourceEncryption::Unencrypted => Some(false),
        _ => None,
    };
    let source = WitnessInput {
        owner,
        global,
        scope,
        encrypted,
    };
    let (links, slots, scans) = source.check_sdk(&archive, &entry)?;
    watch
        .check()
        .map_err(|_| "validate Archive after the SDK witness")?;
    print_line(&format!(
        "Mineable Entity SDK source witness: {}; {links} original links, {slots} signature slots, {scans} scan selections",
        identity.join(" ")
    ));
    Ok(())
}

const fn complete(coverage: &CatalogRecordCoverage) -> bool {
    coverage.unreadable_headers.is_empty()
        && coverage.unreadable_bodies.is_empty()
        && coverage.incomplete_fields.is_empty()
}

struct WitnessInput<'source> {
    owner: RecordTree<'source>,
    global: SignatureLayout<'source>,
    scope: SourceScope,
    encrypted: Option<bool>,
}

impl<'source> WitnessInput<'source> {
    fn check_sdk(
        &self,
        archive: &Archive,
        entry: &DataForgeEntry,
    ) -> TestResult<(usize, usize, usize)> {
        let session = CatalogQuerySession::from_entry(
            archive,
            entry,
            CatalogLanguageSelection::Selected(&[]),
            None,
        )
        .map_err(|_| "prepare the mineable entity SDK session")?
        .with_provenance_mode(ProvenanceMode::Full);
        let sdk = session
            .parsed(None)
            .map_err(|_| "open the parsed Mineable Entity SDK session")?;
        sdk.with_mineable_entities(
            CatalogRecordSelection::RecordIndex(self.owner.record_index()),
            None,
            |snapshot| -> TestResult<(usize, usize, usize)> {
                assert!(
                    snapshot.build() == archive.build()
                        && matches!(snapshot.availability(), DatasetAvailability::Available)
                        && snapshot.requested_languages().is_empty()
                        && snapshot.available_languages().is_empty()
                        && complete(snapshot.coverage())
                        && complete(snapshot.dependency_coverage()),
                    "the selected owner and its radar dependency are readable in this installed Build"
                );
                let [row] = snapshot.rows() else {
                    return Err("the physical RecordIndex yields one Mineable Entity".into());
                };
                let inventory = serde_json::to_value(snapshot.source_inventory())?;
                self.check(row, &inventory)
            },
        ).map_err(|_| "read the targeted Mineable Entity")?
    }

    fn check(
        &self,
        row: &MineableEntity<'_>,
        inventory: &serde_json::Value,
    ) -> TestResult<(usize, usize, usize)> {
        let uses = proof::indexed(inventory, "/uses", "id")?;
        let trace = Trace::of(&self.owner, &uses, &self.scope, self.encrypted);
        let selector = Trace::of(&self.global.tree, &uses, &self.scope, self.encrypted);
        self.check_header(row, &trace)?;
        let Some(raw @ Original::Array(components)) = self.owner.field("Components") else {
            return Err("the original Components array".into());
        };
        assert!(
            row.component_slots.value() == Some(&u32::try_from(components.len())?),
            "the SDK retains the original Components slot count"
        );
        trace.check(
            &serde_json::to_value(&row.component_slots)?,
            raw,
            "selector",
        )?;

        let mining = sample::components(components, MINING);
        let radar = sample::components(components, RADAR);
        assert!(
            mining.count() == row.mining.len()
                && !row.mining.is_empty()
                && radar.count() == row.radar.len()
                && !row.radar.is_empty(),
            "all original occurrences of both component families are present"
        );
        let mut links = 0;
        for ((ordinal, raw, original), output) in
            sample::components(components, MINING).zip(&row.mining)
        {
            check_component(
                ordinal,
                raw,
                &output.source_position,
                &output.source_type,
                &trace,
            )?;
            check_link(original, "composition", &output.composition, &trace)?;
            check_link(original, "globalParams", &output.global_params, &trace)?;
            links += 2;
        }
        let mut slots = 0;
        for ((ordinal, raw, original), output) in
            sample::components(components, RADAR).zip(&row.radar)
        {
            check_component(
                ordinal,
                raw,
                &output.source_position,
                &output.source_type,
                &trace,
            )?;
            let (base, signatures) = radar_base(original, output, &trace)?;
            slots += self.check_scan(base, signatures, &trace, &selector)?;
            links += 1;
        }
        Ok((links, slots, row.radar.len()))
    }

    fn check_header(&self, row: &MineableEntity<'_>, trace: &Trace<'_, 'source>) -> TestResult {
        assert!(
            row.source.record_index().value() == Some(&self.owner.record_index())
                && row.source.guid().value() == Some(&self.owner.guid())
                && row.source.record_path().value() == Some(&self.owner.record_path()),
            "the selected physical owner retains the original header"
        );
        trace.header(
            &serde_json::to_value(row.source.record_index())?,
            "recordIndex",
            "selector",
        )?;
        let serialized = serde_json::to_value(&row.source)?;
        let guid_cell = serialized.get("guid").ok_or("serialized owner GUID")?;
        trace.header(guid_cell, "guid", "payload")
    }

    fn check_scan(
        &self,
        output: &MineableBaseSignature<'_>,
        raw: &Original<'source>,
        trace: &Trace<'_, 'source>,
        selector: &Trace<'_, 'source>,
    ) -> TestResult<usize> {
        let Original::Array(signatures) = raw else {
            return Err("the ordered raw Single array".into());
        };
        let projected = output
            .signatures
            .value()
            .ok_or("the ordered SDK signature slots")?;
        assert!(
            projected.len() == signatures.len() && signatures.len() == self.global.width,
            "signature slots remain aligned with the original declared types"
        );
        trace.check(&serde_json::to_value(&output.signatures)?, raw, "selector")?;
        for (raw, projected) in signatures.iter().zip(projected) {
            let Original::Single(bits) = raw else {
                return Err("the selected signature positions hold Singles".into());
            };
            assert!(
                projected.value().is_some_and(|value| {
                    value.raw_bits() == bits.to_bits()
                        && value.finite().map(f32::to_bits)
                            == bits.is_finite().then(|| bits.to_bits())
                }),
                "each original IEEE Single and its finite view match the same physical signature slot"
            );
            trace.check(&serde_json::to_value(projected)?, raw, "payload")?;
        }
        let selected = signatures
            .get(self.global.slot)
            .ok_or("raw scan signature slot")?;
        let Original::Single(scan) = selected else {
            return Err("the selected raw scan signature is a Single".into());
        };
        assert!(
            output.scan_signature.value().copied().map(f32::to_bits) == Some(scan.to_bits()),
            "the scan chooses the exact signed bits at the original global Locale position"
        );
        let derived = serde_json::to_value(&output.scan_signature)?;
        trace.check(&derived, selected, "payload")?;
        selector.check(&derived, self.global.selector()?, "selector")?;
        Ok(signatures.len())
    }
}

fn check_component<'source>(
    ordinal: usize,
    raw: &Original<'source>,
    position: &SourcedValue<u32>,
    declaration: &SourcedValue<&str>,
    trace: &Trace<'_, 'source>,
) -> TestResult {
    let Original::Struct(original) = raw else {
        return Err("a retained original component struct".into());
    };
    assert!(
        position.value() == Some(&u32::try_from(ordinal)?)
            && declaration.value() == Some(&original.type_name()),
        "the component retains its exact physical position and declaration"
    );
    trace.check(&serde_json::to_value(position)?, raw, "conversion")?;
    trace.part(
        &serde_json::to_value(declaration)?,
        raw,
        "$type",
        "selector",
    )
}

fn check_link<'source>(
    original: &StructValue<'source>,
    field: &str,
    cell: &SourcedValue<CatalogRecordReference>,
    trace: &Trace<'_, 'source>,
) -> TestResult {
    let Some(raw @ Original::Reference(original)) = original.field(field) else {
        return Err("the selected original field is a resolved Reference".into());
    };
    let reference = cell.value().ok_or("the selected SDK link")?;
    assert!(
        reference.guid().value() == Some(&original.guid())
            && reference.target_record_index().value() == Some(&original.record_index())
            && reference
                .target_type()
                .value()
                .is_some_and(|name| name == original.type_name()),
        "the SDK link retains the original GUID and physical target index and type"
    );
    let exported = serde_json::to_value(cell)?;
    trace.check(&exported, raw, "selector")?;
    let serialized = serde_json::to_value(reference)?;
    let guid_cell = serialized.get("guid").ok_or("serialized reference GUID")?;
    trace.check(guid_cell, raw, "payload")?;
    trace.header_for(
        &serde_json::to_value(reference.target_record_index())?,
        original.record_index(),
        "recordIndex",
        "selector",
    )
}

fn radar_base<'output, 'sdk, 'original, 'source>(
    original: &'original StructValue<'source>,
    output: &'output MineableSignature<'sdk>,
    trace: &Trace<'_, 'source>,
) -> TestResult<(
    &'output MineableBaseSignature<'sdk>,
    &'original Original<'source>,
)> {
    let Some(Original::Struct(properties)) = original.field("radarProperties") else {
        return Err("original radar properties".into());
    };
    let Some(Original::Struct(base)) = properties.field("baseSignatureParams") else {
        return Err("original radar base signature".into());
    };
    let projected = output
        .properties
        .value()
        .ok_or("the SDK radar properties")?;
    check_link(properties, "contactType", &projected.contact_type, trace)?;
    let signatures = base.field("signatures").ok_or("original signature array")?;
    let output = projected
        .base_signature_params
        .value()
        .ok_or("the SDK base signature")?;
    Ok((output, signatures))
}
