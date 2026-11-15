// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use crate::key::Key;
use crate::{LocalizationError, LocalizationLimits};

/// The texts of one key: the line `key=` and the line `key,suffix=`.
#[derive(Debug, Clone, Copy, Default)]
struct Texts<'a> {
    plain: Option<&'a str>,
    suffixed: Option<&'a str>,
}

impl<'a> Texts<'a> {
    fn preferred(self) -> Option<&'a str> {
        match (self.plain, self.suffixed) {
            (Some(plain), _) if !plain.is_empty() => Some(plain),
            (_, Some(suffixed)) if !suffixed.is_empty() => Some(suffixed),
            (plain, suffixed) => plain.or(suffixed),
        }
    }
}

/// The texts of one language's `global.ini`, indexed by localisation key.
///
/// Every line `key=value` holds one text; the key ends at the first `=` or `,` (so `ASD_01,P`
/// is the key `ASD_01`) and the value is the rest of the line, kept as the game stores it. Lines
/// without `=` or with an empty key are skipped without ending the read, and of two lines with
/// the same key the later one wins. A key with a comma suffix keeps its own text beside the
/// plain key's: [`GlobalIni::resolve`] prefers the plain key's text and
/// [`GlobalIni::resolve_suffixed`] returns the other. Texts are borrowed from the parsed text.
#[derive(Debug, Clone)]
pub struct GlobalIni<'a> {
    texts: HashMap<Key<'a>, Texts<'a>>,
    skipped: usize,
}

impl<'a> GlobalIni<'a> {
    /// Indexes the `global.ini` text `text`; a leading byte order mark is ignored.
    ///
    /// Use [`IniText::decode`](crate::IniText::decode) to get the text of bytes in any of the
    /// encodings the game's files come in.
    ///
    /// # Errors
    ///
    /// Returns [`LocalizationError::OutOfMemory`] when index storage cannot be reserved.
    pub fn parse(text: &'a str) -> Result<Self, LocalizationError> {
        Self::parse_with_limits(text, LocalizationLimits::UNLIMITED)
    }

    /// Indexes the text with the caller's decoded byte and distinct key budgets.
    ///
    /// # Errors
    ///
    /// Returns a budget error when a configured budget is exceeded or
    /// [`LocalizationError::OutOfMemory`] when index storage cannot be reserved.
    pub fn parse_with_limits(
        text: &'a str,
        limits: LocalizationLimits,
    ) -> Result<Self, LocalizationError> {
        let text = text.trim_prefix('\u{feff}');
        limits.check_decoded_bytes(text.len())?;
        let mut texts = HashMap::new();
        let mut skipped = 0;
        for line in text.split('\n') {
            let line = line.trim_suffix('\r');
            let key = line.split_once('=').and_then(|(key, value)| {
                let (key, suffixed) = key
                    .split_once(',')
                    .map_or((key, false), |(key, _)| (key, true));
                let key = key.trim();
                (!key.is_empty()).then_some((key, value, suffixed))
            });
            let Some((key, value, suffixed)) = key else {
                skipped += usize::from(!line.trim().is_empty());
                continue;
            };
            let slot = text_slot(&mut texts, Key(key), limits.keys())?;
            if suffixed {
                slot.suffixed = Some(value);
            } else {
                slot.plain = Some(value);
            }
        }
        Ok(Self { texts, skipped })
    }

    /// Returns the text of the localisation `key`, with or without its leading `@`, ignoring
    /// ASCII case; `None` when the file has no such key.
    ///
    /// When the file holds both `key=` and `key,P=`, the plain key's text is returned, unless it
    /// is empty and the suffixed one is not.
    #[must_use]
    pub fn resolve(&self, key: &str) -> Option<&'a str> {
        self.texts_of(key)?.preferred()
    }

    /// Returns the text of the line `key,suffix=` of the localisation `key`, with or without its
    /// leading `@`, ignoring ASCII case; `None` when the file has no such line.
    #[must_use]
    pub fn resolve_suffixed(&self, key: &str) -> Option<&'a str> {
        self.texts_of(key)?.suffixed
    }

    fn texts_of(&self, key: &str) -> Option<Texts<'a>> {
        let key = key.trim_prefix('@');
        let texts: &HashMap<Key<'_>, Texts<'a>> = &self.texts;
        texts.get(&Key(key)).copied()
    }

    pub(crate) fn contains_key(&self, key: &str) -> bool {
        self.texts.contains_key(&Key(key))
    }

    /// Returns every key with its text, in no particular order; a key is spelled as its first
    /// line spells it.
    pub fn iter(&self) -> impl Iterator<Item = (&'a str, &'a str)> {
        self.texts
            .iter()
            .filter_map(|(key, texts)| Some((key.0, texts.preferred()?)))
    }

    /// Returns the number of keys.
    #[must_use]
    pub fn len(&self) -> usize {
        self.texts.len()
    }

    /// Returns whether the file holds no key.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.texts.is_empty()
    }

    /// Returns the number of non-blank lines that were skipped for having no `=` or an empty
    /// key.
    #[must_use]
    pub const fn skipped(&self) -> usize {
        self.skipped
    }
}

fn text_slot<'index, 'source>(
    texts: &'index mut HashMap<Key<'source>, Texts<'source>>,
    key: Key<'source>,
    limit: Option<usize>,
) -> Result<&'index mut Texts<'source>, LocalizationError> {
    let count = texts.len();
    if (limit == Some(count) || count == texts.capacity()) && !texts.contains_key(&key) {
        if let Some(limit) = limit
            && count == limit
        {
            return Err(LocalizationError::TooManyKeys { limit });
        }
        texts
            .try_reserve(1)
            .map_err(|_| LocalizationError::OutOfMemory {
                keys: count.saturating_add(1),
            })?;
    }
    Ok(texts.entry(key).or_default())
}
