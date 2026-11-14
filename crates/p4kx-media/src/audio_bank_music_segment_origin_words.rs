// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourceUseRole;

use crate::MediaError;
use crate::audio_bank_origins::BankOrigins;

use super::{List, MusicSites, field, word};

enum Target {
    Field(&'static str),
    Numeric(&'static str),
}

struct HeaderWord {
    target: Target,
    name: &'static str,
    range: Option<[u64; 2]>,
    before: Option<List>,
}

impl HeaderWord {
    const fn new(
        target: Target,
        name: &'static str,
        range: Option<[u64; 2]>,
        before: Option<List>,
    ) -> Self {
        Self {
            target,
            name,
            range,
            before,
        }
    }
}

impl MusicSites<'_> {
    pub(super) fn node_words(
        &self,
        prepared: &mut BankOrigins<'_, '_, '_>,
    ) -> Result<(), MediaError> {
        self.write_words(
            prepared,
            [HeaderWord::new(
                Target::Field("child_count"),
                "musicChildCount",
                self.children,
                None,
            )],
        )?;
        self.write_words(prepared, self.meter_words())?;
        self.write_words(
            prepared,
            [HeaderWord::new(
                Target::Field("stinger_count"),
                "musicStingerCount",
                self.stingers,
                Some(List::Children),
            )],
        )
    }

    pub(super) fn segment_words(
        &self,
        prepared: &mut BankOrigins<'_, '_, '_>,
    ) -> Result<(), MediaError> {
        self.write_words(
            prepared,
            [
                HeaderWord::new(
                    Target::Numeric("duration"),
                    "musicDuration",
                    self.segment
                        .and_then(|segment| segment.duration.as_ref())
                        .map(word),
                    Some(List::Stingers),
                ),
                HeaderWord::new(
                    Target::Field("marker_count"),
                    "musicMarkerCount",
                    self.markers,
                    Some(List::Stingers),
                ),
            ],
        )
    }

    fn meter_words(&self) -> [HeaderWord; 5] {
        let node = self.node_fields;
        let before = Some(List::Children);
        [
            HeaderWord::new(
                Target::Numeric("grid_period"),
                "musicGridPeriod",
                node.grid_period.as_ref().map(word),
                before,
            ),
            HeaderWord::new(
                Target::Numeric("grid_offset"),
                "musicGridOffset",
                node.grid_offset.as_ref().map(word),
                before,
            ),
            HeaderWord::new(
                Target::Numeric("tempo"),
                "musicTempo",
                node.tempo.as_ref().map(word),
                before,
            ),
            HeaderWord::new(
                Target::Field("beats_per_bar"),
                "musicBeatsPerBar",
                node.beats_per_bar.as_ref().map(word),
                before,
            ),
            HeaderWord::new(
                Target::Field("beat_value"),
                "musicBeatValue",
                node.beat_value.as_ref().map(word),
                before,
            ),
        ]
    }

    fn write_words(
        &self,
        prepared: &mut BankOrigins<'_, '_, '_>,
        words: impl IntoIterator<Item = HeaderWord>,
    ) -> Result<(), MediaError> {
        for word in words {
            let origin = prepared.capture(
                self.preceding(word.before).chain(
                    word.range
                        .as_ref()
                        .map(|span| field(span, word.name, SourceUseRole::Payload)),
                ),
            )?;
            match word.target {
                Target::Field(key) => prepared.field(&format!("/data/payload/{key}"), origin)?,
                Target::Numeric(key) => {
                    prepared.subtree(&format!("/data/payload/{key}"), origin)?;
                }
            }
        }
        Ok(())
    }
}
