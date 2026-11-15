// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::archive_animation_path;
use schemars::JsonSchema;
use serde::Serialize;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[schemars(rename = "AnimationWildcardFamily")]
pub(crate) enum Family {
    Caf,
    BlendSpace,
    CombinedSpace,
}

impl Family {
    const fn of(extension: &str) -> Option<Self> {
        if extension.eq_ignore_ascii_case("caf") {
            Some(Self::Caf)
        } else if extension.eq_ignore_ascii_case("bspace") {
            Some(Self::BlendSpace)
        } else if extension.eq_ignore_ascii_case("comb") {
            Some(Self::CombinedSpace)
        } else {
            None
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) enum Scope<'source> {
    Missing,
    Malformed,
    Reset,
    Authored(&'source str),
}

impl<'source> Scope<'source> {
    fn folder(self) -> Result<&'source str, Unexpanded> {
        match self {
            Self::Missing => Err(Unexpanded::MissingFilepath),
            Self::Malformed => Err(Unexpanded::MalformedFilepath),
            Self::Reset => Err(Unexpanded::ResetFilepath),
            Self::Authored(path) => Ok(path.trim_end_matches(['/', '\\'])),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[schemars(rename = "AnimationWildcardUnexpandedReason")]
pub(crate) enum Unexpanded {
    UnsupportedPattern,
    MissingFilepath,
    MalformedFilepath,
    ResetFilepath,
    InvalidFilepath,
}

#[derive(Debug)]
pub(crate) struct Rule<'source> {
    folder: String,
    stem_prefix: &'source str,
    alias_prefix: &'source str,
    family: Family,
    recursive: bool,
    uses_scope: bool,
}

impl<'source> Rule<'source> {
    pub(crate) fn parse(
        name: &'source str,
        path: &'source str,
        scope: Scope<'_>,
    ) -> Result<Self, Unexpanded> {
        let alias_prefix = terminal_star(name).ok_or(Unexpanded::UnsupportedPattern)?;
        if name.starts_with(['#', '$']) || alias_prefix.contains(['/', '\\']) {
            return Err(Unexpanded::UnsupportedPattern);
        }
        let pattern = FilePattern::parse(path)?;
        let uses_scope = !rooted(pattern.folder);
        let folder = pattern.archive_folder(uses_scope, scope)?;
        Ok(Self {
            folder,
            stem_prefix: pattern.stem_prefix,
            alias_prefix,
            family: pattern.family,
            recursive: pattern.recursive,
            uses_scope,
        })
    }

    pub(crate) const fn family(&self) -> Family {
        self.family
    }

    pub(crate) const fn uses_scope(&self) -> bool {
        self.uses_scope
    }

    pub(crate) fn alias(&self, captured: &str) -> String {
        format!("{}{captured}", self.alias_prefix)
    }

    pub(crate) fn folder(&self) -> &str {
        &self.folder
    }

    pub(crate) fn includes_folder(&self, folder: &str) -> bool {
        if self.recursive {
            folder.starts_with(&self.folder)
        } else {
            folder == self.folder
        }
    }

    pub(crate) fn capture<'entry>(&self, path: &'entry str) -> Option<&'entry str> {
        let file = path.rsplit(['/', '\\']).next()?;
        let (stem, extension) = file.rsplit_once('.')?;
        if stem.is_empty() || Family::of(extension)? != self.family {
            return None;
        }
        let (prefix, capture) = stem.split_at_checked(self.stem_prefix.len())?;
        prefix
            .eq_ignore_ascii_case(self.stem_prefix)
            .then_some(capture)
    }
}

struct FilePattern<'source> {
    folder: &'source str,
    stem_prefix: &'source str,
    extension: &'source str,
    family: Family,
    recursive: bool,
}

impl<'source> FilePattern<'source> {
    fn parse(path: &'source str) -> Result<Self, Unexpanded> {
        if path.starts_with(['/', '\\']) {
            return Err(Unexpanded::UnsupportedPattern);
        }
        let (folder, file) = path.rsplit_once(['/', '\\']).unwrap_or(("", path));
        let (folder, recursive) = match folder.rsplit_once(['/', '\\']) {
            Some((literal, "*")) if !literal.is_empty() => (literal, true),
            _ if folder == "*" => ("", true),
            _ => (folder, false),
        };
        if folder.contains(['*', '?']) {
            return Err(Unexpanded::UnsupportedPattern);
        }
        let (stem, extension) = file
            .rsplit_once('.')
            .ok_or(Unexpanded::UnsupportedPattern)?;
        let stem_prefix = terminal_star(stem).ok_or(Unexpanded::UnsupportedPattern)?;
        let family = Family::of(extension).ok_or(Unexpanded::UnsupportedPattern)?;
        Ok(Self {
            folder,
            stem_prefix,
            extension,
            family,
            recursive,
        })
    }

    fn archive_folder(&self, uses_scope: bool, scope: Scope<'_>) -> Result<String, Unexpanded> {
        let mut literal = String::new();
        if !self.folder.is_empty() {
            literal.push_str(self.folder);
            literal.push('/');
        }
        literal.push_str(self.stem_prefix);
        literal.push('.');
        literal.push_str(self.extension);
        if !uses_scope {
            return archive_animation_path(&literal)
                .map(directory_key)
                .ok_or(Unexpanded::UnsupportedPattern);
        }
        let scope = scope.folder()?;
        if !rooted(scope) {
            return Err(Unexpanded::InvalidFilepath);
        }
        if let Some(path) = archive_animation_path(&format!("{scope}/{literal}")) {
            return Ok(directory_key(path));
        }
        Err(
            if archive_animation_path(&format!("Data/{literal}")).is_some() {
                Unexpanded::InvalidFilepath
            } else {
                Unexpanded::UnsupportedPattern
            },
        )
    }
}

pub(crate) fn candidate_folder(path: &str) -> Option<String> {
    let (root, _) = path.split_once(['/', '\\'])?;
    if !root.eq_ignore_ascii_case("data") {
        return None;
    }
    let file = path.rsplit(['/', '\\']).next()?;
    let (stem, extension) = file.rsplit_once('.')?;
    if stem.is_empty() {
        return None;
    }
    Family::of(extension)?;
    archive_animation_path(path).map(directory_key)
}

fn terminal_star(pattern: &str) -> Option<&str> {
    let prefix = pattern.strip_suffix('*')?;
    (!prefix.contains(['*', '?'])).then_some(prefix)
}

fn rooted(path: &str) -> bool {
    path.split(['/', '\\']).next().is_some_and(|root| {
        root.eq_ignore_ascii_case("data") || root.eq_ignore_ascii_case("animations")
    })
}

fn directory_key(mut path: String) -> String {
    if let Some(separator) = path.rfind('/') {
        path.truncate(separator + 1);
    }
    path.make_ascii_lowercase();
    path
}

#[cfg(test)]
mod tests {
    use super::{Family, Rule, Scope, Unexpanded, candidate_folder};

    #[test]
    fn capture_uses_only_the_wildcarded_filename_stem_and_original_case() {
        let rule = Rule::parse(
            "Test_*",
            r"TestFolder\HighJump_*.caf",
            Scope::Authored(r"Animations\Human\"),
        )
        .expect("documented CAF pattern");
        assert_eq!(rule.family(), Family::Caf);
        assert!(rule.uses_scope());
        let actual = r"Data/Animations/HUMAN\TestFolder\highJUMP_Étage.CAF";
        let folder = candidate_folder(actual).expect("safe candidate folder");
        assert!(rule.includes_folder(&folder));
        assert_eq!(
            rule.alias(rule.capture(actual).expect("matching CAF")),
            "Test_Étage"
        );
        assert!(!rule.includes_folder("data/animations/human/testfolder/deeper/"));
        assert!(rule.capture("HighJump_02.BSPACE").is_none());
        assert!(rule.capture("highJUMPing.caf").is_none());
    }

    #[test]
    fn terminal_directory_star_includes_the_base_and_nested_folders() {
        let rule = Rule::parse(
            "_*",
            r"lmg\*\*.COMB",
            Scope::Authored("Data/Animations/Actor"),
        )
        .expect("documented recursive pattern");
        assert_eq!(rule.family(), Family::CombinedSpace);
        assert_eq!(rule.folder(), "data/animations/actor/lmg/");
        assert!(rule.includes_folder(rule.folder()));
        assert!(rule.includes_folder("data/animations/actor/lmg/turns/slow/"));
        assert!(!rule.includes_folder("data/animations/actor/lmg-other/"));
        assert_eq!(
            rule.alias(rule.capture("MoveTurn.COMB").expect("matching COMB")),
            "_MoveTurn"
        );
    }

    #[test]
    fn rooted_patterns_ignore_scope_and_invalid_inputs_keep_their_reason() {
        let rooted = Rule::parse("*", r"Animations\*.bspace", Scope::Malformed)
            .expect("rooted blend-space pattern");
        assert_eq!(rooted.family(), Family::BlendSpace);
        assert!(!rooted.uses_scope());
        assert_eq!(rooted.folder(), "data/animations/");
        for (scope, reason) in [
            (Scope::Missing, Unexpanded::MissingFilepath),
            (Scope::Malformed, Unexpanded::MalformedFilepath),
            (Scope::Reset, Unexpanded::ResetFilepath),
            (
                Scope::Authored("Data/../Animations"),
                Unexpanded::InvalidFilepath,
            ),
        ] {
            assert_eq!(Rule::parse("*", "*.caf", scope).unwrap_err(), reason);
        }
        for path in [
            "*/lmg/*.caf",
            "../*.caf",
            "*.?af",
            "*high*.caf",
            "high?.caf",
        ] {
            assert_eq!(
                Rule::parse("*", path, Scope::Authored("Animations")).unwrap_err(),
                Unexpanded::UnsupportedPattern,
            );
        }
        assert!(candidate_folder("Data/Animations/../Drive.caf").is_none());
    }
}
