// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_extraction::SourceUseRole;
use p4kx_extraction::bank::{
    BankHircField, BankHircSound, BankSoundContinuation, BankSoundFields, BankSoundRead,
};

use crate::audio_bank_inventory::VersionSite;
use crate::audio_bank_origins::{BankOrigins, EvidenceSite, complete_directory, section_routing};
use crate::audio_bank_sound_node_origins;
use crate::origin_capture::OriginSite;
use crate::origin_input::MediaOriginInput;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::{MediaError, MediaWriter};

pub(crate) struct SoundSites {
    object: [u64; 2],
    body: [u64; 1],
    header: [u64; 1],
    pub(crate) plugin: Scalar,
    storage: Scalar,
    source_id: Scalar,
}

pub(crate) struct Scalar {
    range: Option<[u64; 2]>,
    name: &'static str,
}

pub(crate) fn prepare(
    input: &MediaOriginInput,
    sound: &BankHircSound,
    version: VersionSite,
    writer: &MediaWriter<'_>,
) -> Result<PreparedRecordOrigins, MediaError> {
    let object = &sound.object;
    let sites = SoundSites::new(sound, version);
    let mut prepared = BankOrigins::new(input, writer)?;
    let routing = prepared.capture([sites.route()])?;
    prepared.indices(routing)?;
    let section = prepared.capture([section_routing(&[object.section_ordinal()])])?;
    prepared.field("/subresource/indices/0", section)?;
    sites.fields(&mut prepared, sound)?;
    Ok(prepared.finish())
}

impl SoundSites {
    pub(crate) fn new(sound: &BankHircSound, version: VersionSite) -> Self {
        let object = &sound.object;
        Self {
            object: [object.section_ordinal(), object.ordinal()],
            body: [object.body_offset()],
            header: [version.body_offset],
            plugin: Scalar::new(sound.fields.plugin_id.as_ref(), "soundPluginId"),
            storage: Scalar::new(sound.fields.storage_code.as_ref(), "soundStorage"),
            source_id: Scalar::new(sound.fields.source_id.as_ref(), "soundSourceId"),
        }
    }

    fn fields(
        &self,
        prepared: &mut BankOrigins<'_, '_, '_>,
        sound: &BankHircSound,
    ) -> Result<(), MediaError> {
        if matches!(sound.status, BankSoundRead::Unsupported(_)) {
            let unsupported = prepared.capture(self.base())?;
            prepared.field("/data/payload/source", unsupported.clone())?;
            prepared.field("/data/payload/continuation", unsupported.clone())?;
            prepared.field("/data/payload/layout", unsupported.clone())?;
            return prepared.subtree("/issues", unsupported);
        }
        self.source(prepared, sound)?;
        let bound = prepared.capture(self.base().into_iter().chain([self.extent()]))?;
        prepared.field("/data/payload/continuation/kind", bound.clone())?;
        if matches!(
            sound.fields.continuation,
            BankSoundContinuation::Opaque { .. }
        ) {
            prepared.range("/data/payload/continuation/opaque_range", &bound, &bound)?;
        }
        let layout = sound.fields.node.as_ref().map_or(Ok(bound), |node| {
            audio_bank_sound_node_origins::project(self, prepared, node)
        })?;
        prepared.field("/data/payload/layout", layout.clone())?;
        if matches!(sound.status, BankSoundRead::Malformed(_)) {
            prepared.subtree("/issues", layout)?;
        }
        Ok(())
    }

    fn source(
        &self,
        prepared: &mut BankOrigins<'_, '_, '_>,
        sound: &BankHircSound,
    ) -> Result<(), MediaError> {
        let fields: &BankSoundFields = &sound.fields;
        let storage = &self.storage;
        let bits = Scalar::new(fields.source_bits.as_ref(), "soundSourceBits");
        for (path, word) in [
            ("/data/payload/source/plugin_id", &self.plugin),
            ("/data/payload/source/storage_code", storage),
            ("/data/payload/source/source_id", &self.source_id),
            (
                "/data/payload/source/cache_id",
                &Scalar::new(fields.cache_id.as_ref(), "soundCacheId"),
            ),
            (
                "/data/payload/source/in_memory_size",
                &Scalar::new(fields.in_memory_size.as_ref(), "soundMemorySize"),
            ),
            ("/data/payload/source/source_bits", &bits),
        ] {
            self.field(prepared, path, word, SourceUseRole::Payload)?;
        }
        if matches!(sound.status, BankSoundRead::Decoded(_)) {
            self.derived(prepared, storage, &bits)?;
        }
        Ok(())
    }

    fn derived(
        &self,
        prepared: &mut BankOrigins<'_, '_, '_>,
        storage: &Scalar,
        bits: &Scalar,
    ) -> Result<(), MediaError> {
        for (path, word) in [
            ("/data/payload/source/storage", storage),
            ("/data/payload/source/language_specific", bits),
            ("/data/payload/source/source_plugin", &self.plugin),
            ("/data/payload/source/external_source", &self.plugin),
            ("/data/payload/source/midi", &self.plugin),
            ("/data/payload/source/has_media_file", &self.plugin),
        ] {
            self.field(prepared, path, word, SourceUseRole::Conversion)?;
        }
        Ok(())
    }

    fn field(
        &self,
        prepared: &mut BankOrigins<'_, '_, '_>,
        path: &str,
        word: &Scalar,
        role: SourceUseRole,
    ) -> Result<(), MediaError> {
        let selection = word.site(role).unwrap_or_else(|| self.extent());
        let evidence = prepared.capture(self.base().into_iter().chain([selection]))?;
        prepared.field(path, evidence)
    }

    pub(crate) fn media_id(&self) -> Result<EvidenceSite<'_>, MediaError> {
        self.source_id
            .site(SourceUseRole::Selector)
            .ok_or_else(|| MediaError::Contract("decoded Sound lacks its original media ID".into()))
    }

    pub(crate) fn media_selectors(&self) -> Result<[EvidenceSite<'_>; 3], MediaError> {
        let [Some(storage), Some(plugin)] =
            [&self.storage, &self.plugin].map(|word| word.site(SourceUseRole::Selector))
        else {
            return Err(MediaError::Contract(
                "decoded Sound lacks its original storage or plugin selector".into(),
            ));
        };
        Ok([self.media_id()?, storage, plugin])
    }

    pub(crate) const fn route(&self) -> EvidenceSite<'_> {
        (
            OriginSite::new("bnkHircPrefix", &self.object, "objectRouting"),
            SourceUseRole::Selector,
        )
    }

    pub(crate) const fn base(&self) -> [EvidenceSite<'_>; 4] {
        [
            self.route(),
            complete_directory("bankVersionSelection"),
            (
                OriginSite::new("bnkHircObject", &self.body, "typeCode"),
                SourceUseRole::Selector,
            ),
            (
                OriginSite::new("bnkBkhd", &self.header, "version"),
                SourceUseRole::Selector,
            ),
        ]
    }

    pub(crate) const fn extent(&self) -> EvidenceSite<'_> {
        (
            OriginSite::new("bnkHircObject", &self.body, "bodyLength"),
            SourceUseRole::Selector,
        )
    }
}

impl Scalar {
    pub(crate) fn new<T>(field: Option<&BankHircField<T>>, name: &'static str) -> Self {
        Self {
            range: field.map(|word| coordinates(&word.range)),
            name,
        }
    }

    pub(crate) fn site(&self, role: SourceUseRole) -> Option<EvidenceSite<'_>> {
        self.range
            .as_ref()
            .map(|range| (OriginSite::new("bnkHircField", range, self.name), role))
    }
}

pub(crate) const fn coordinates(range: &Range<u64>) -> [u64; 2] {
    [range.start, range.end]
}
