// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The animation names of a Mannequin animation database (`.adb`) and of the databases it nests.

use std::collections::HashSet;
use std::sync::Arc;

use p4kx_extraction::{EntryOrigin, SourceLocation};

use crate::loadout_xml::{AttributeNamespaces, Node, Scope};
use crate::markup::{self, Budget, Element};
use crate::{ModelFailure, ModelProvider};

const MAX_DATABASE_BYTES: u64 = 16 << 20;

const MAX_DATABASES: usize = 64;

const MAX_NAMES: usize = 1 << 16;

/// The animation names of a database and what could not be read of it or of its nested databases.
#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct Clips {
    /// The names in the order of their first use, each once, ignoring ASCII case.
    pub(super) names: Vec<String>,
    pub(super) selectors: Vec<NameSelector>,
    pub(super) unpositioned_selectors: bool,
    /// The nested databases that could not be read, each with why.
    pub(super) unread: Vec<(String, String)>,
    /// What was dropped because a limit was reached, each as a sentence for a report.
    pub(super) truncated: Vec<String>,
}

#[derive(Debug)]
pub(super) struct NameSelector {
    pub(super) name: String,
    pub(super) input: EntryOrigin,
    pub(super) position: SourceLocation,
    pub(super) route: Route,
}

impl PartialEq for NameSelector {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
            && self.input.binding() == other.input.binding()
            && self.position == other.position
            && self.route == other.route
    }
}

impl Eq for NameSelector {}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) enum Route {
    #[default]
    Root,
    Nested(Arc<NestedRoute>),
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct NestedRoute {
    pub(super) parent: Route,
    pub(super) selector: Option<SelectorOrigin>,
}

#[derive(Debug)]
pub(super) struct SelectorOrigin {
    pub(super) input: EntryOrigin,
    pub(super) position: SourceLocation,
}

impl PartialEq for SelectorOrigin {
    fn eq(&self, other: &Self) -> bool {
        self.input.binding() == other.input.binding() && self.position == other.position
    }
}

impl Eq for SelectorOrigin {}

const NAMESPACES: AttributeNamespaces = AttributeNamespaces {
    text_bytes: "mannequinAnimationTextBytes",
    cryxml_attribute: "mannequinAnimationCryXmlAttribute",
};

/// Why the database an Entity names gives no clips.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum DatabaseProblem {
    /// The Archive has no such file.
    Missing,
    /// The file is too large or is no database; the text says why.
    Unreadable(String),
}

/// Reads the animation names of the database at `path`, a path below `Data`, and of the databases
/// its `SubADB` elements name, breadth first, within [`MAX_DATABASES`] and [`MAX_NAMES`].
///
/// # Errors
///
/// Returns why the database at `path` itself cannot be read; a nested database that cannot be read
/// is listed in [`Clips::unread`], and what a limit dropped in [`Clips::truncated`].
pub(super) fn clips(models: &dyn ModelProvider, path: &str) -> Result<Clips, DatabaseProblem> {
    let mut clips = Clips::default();
    let mut seen_names = HashSet::new();
    let mut seen_files: HashSet<String> = HashSet::from([key(path)]);
    let mut queue = vec![(path.to_owned(), Route::Root)];
    let (mut next, mut databases_dropped, mut names_dropped) = (0, false, false);
    while let Some((file, route)) = queue.get(next).cloned() {
        let nested = match read(models, &file, &route, &mut clips, &mut seen_names) {
            Ok(found) => {
                names_dropped |= found.names_dropped;
                found.databases
            }
            Err(problem) if next == 0 => return Err(problem),
            Err(problem) => {
                clips.unread.push((file, describe(&problem)));
                Vec::new()
            }
        };
        for (file, selector) in nested {
            if !seen_files.insert(key(&file)) {
                continue;
            }
            if queue.len() < MAX_DATABASES {
                queue.push((
                    file,
                    Route::Nested(Arc::new(NestedRoute {
                        parent: route.clone(),
                        selector,
                    })),
                ));
            } else {
                databases_dropped = true;
            }
        }
        next += 1;
    }
    if databases_dropped {
        clips.truncated.push(format!(
            "only the first {MAX_DATABASES} animation databases are read, the others are left out"
        ));
    }
    if names_dropped {
        clips.truncated.push(format!(
            "only the first {MAX_NAMES} animation names of the databases are kept"
        ));
    }
    Ok(clips)
}

fn key(path: &str) -> String {
    path.replace('\\', "/").to_ascii_lowercase()
}

fn describe(problem: &DatabaseProblem) -> String {
    match problem {
        DatabaseProblem::Missing => "it is not in the Archive".to_owned(),
        DatabaseProblem::Unreadable(reason) => reason.clone(),
    }
}

fn read(
    models: &dyn ModelProvider,
    path: &str,
    route: &Route,
    output: &mut Clips,
    seen: &mut HashSet<String>,
) -> Result<Found, DatabaseProblem> {
    let (origin, bytes) = models.file_observed(path, MAX_DATABASE_BYTES);
    let bytes = bytes.map_err(|failure| match failure {
        ModelFailure::Missing => DatabaseProblem::Missing,
        other => DatabaseProblem::Unreadable(other.to_string()),
    })?;
    let mut budget = Budget::new(bytes.len());
    markup::parse_with_source(&bytes, &mut budget, |root, original, scope| {
        Ok(collect(
            &root,
            original,
            scope,
            origin.as_ref(),
            route,
            output,
            seen,
        ))
    })
    .map_err(DatabaseProblem::Unreadable)
}

struct Found {
    databases: Vec<(String, Option<SelectorOrigin>)>,
    names_dropped: bool,
}

fn collect(
    root: &Element,
    original: Node<'_, '_>,
    scope: &Scope<'_, '_>,
    origin: Option<&EntryOrigin>,
    route: &Route,
    output: &mut Clips,
    seen: &mut HashSet<String>,
) -> Found {
    let mut found = Found {
        databases: Vec::new(),
        names_dropped: false,
    };
    let mut stack = vec![(root, original)];
    while let Some((element, source)) = stack.pop() {
        if element.name.eq_ignore_ascii_case("Animation")
            && let Some(name) = element.attribute("name").filter(|name| !name.is_empty())
        {
            if admit_name(name, output, seen) {
                if let Some(SelectorOrigin { input, position }) =
                    selector_origin(origin, scope, source, "name")
                {
                    output.selectors.push(NameSelector {
                        name: name.to_owned(),
                        input,
                        position,
                        route: route.clone(),
                    });
                } else if origin.is_some() {
                    output.unpositioned_selectors = true;
                }
            } else {
                found.names_dropped = true;
            }
        }
        if element.name.eq_ignore_ascii_case("SubADB")
            && let Some(file) = element.attribute("File").filter(|file| !file.is_empty())
        {
            found.databases.push((
                file.to_owned(),
                selector_origin(origin, scope, source, "File"),
            ));
        }
        stack.extend(element.children.iter().zip(source.children()).rev());
    }
    found
}

fn admit_name(name: &str, output: &mut Clips, seen: &mut HashSet<String>) -> bool {
    let key = name.to_ascii_lowercase();
    if output.names.len() == MAX_NAMES {
        return seen.contains(&key);
    }
    if seen.insert(key) {
        output.names.push(name.to_owned());
    }
    true
}

fn selector_origin(
    origin: Option<&EntryOrigin>,
    scope: &Scope<'_, '_>,
    source: Node<'_, '_>,
    field: &str,
) -> Option<SelectorOrigin> {
    Some(SelectorOrigin {
        input: origin?.clone(),
        position: NAMESPACES.location(&scope.attribute_ascii_position(source, field)?, field)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ProvidedModel;

    struct Files(Vec<(String, String)>);

    fn files(list: &[(&str, &str)]) -> Files {
        Files(
            list.iter()
                .map(|(name, text)| ((*name).to_owned(), (*text).to_owned()))
                .collect(),
        )
    }

    impl ModelProvider for Files {
        fn model(&self, _: &str, _: u32) -> Result<ProvidedModel, ModelFailure> {
            Err(ModelFailure::Missing)
        }

        fn library(&self, _: &str, _: &str) -> p4kx_model::MaterialStatus {
            p4kx_model::MaterialStatus::Unnamed
        }

        fn file(&self, path: &str, _: u64) -> Result<Vec<u8>, ModelFailure> {
            self.0
                .iter()
                .find(|(name, _)| name.eq_ignore_ascii_case(path))
                .map(|(_, text)| text.as_bytes().to_vec())
                .ok_or(ModelFailure::Missing)
        }
    }

    const ROOT: &str = r#"<AnimDB>
        <SubADBs><SubADB File="Animations/sub.adb"/><SubADB File="animations\gone.adb"/></SubADBs>
        <FragmentList>
          <Idle><Fragment><AnimLayer><Animation name="idle_01"/></AnimLayer></Fragment>
                <Fragment><AnimLayer><Animation name="IDLE_01"/><Animation name="walk"/></AnimLayer></Fragment></Idle>
        </FragmentList></AnimDB>"#;

    #[test]
    fn names_are_listed_once_in_order_of_use_with_those_of_nested_databases_after() {
        let files = files(&[
            ("a.adb", ROOT),
            (
                "Animations/sub.adb",
                r#"<AnimDB><FragmentList><X><Fragment><AnimLayer><Animation name="walk"/><Animation name="roar"/></AnimLayer></Fragment></X></FragmentList></AnimDB>"#,
            ),
        ]);

        let clips = clips(&files, "a.adb").expect("clips");

        assert_eq!(clips.names, ["idle_01", "walk", "roar"]);
        assert_eq!(clips.unread.len(), 1);
        assert_eq!(clips.unread[0].0, "animations\\gone.adb");
        assert_eq!(clips.truncated, Vec::<String>::new());
    }

    #[test]
    fn a_database_that_names_itself_is_read_once() {
        let files = files(&[(
            "a.adb",
            r#"<AnimDB><SubADBs><SubADB File="A.adb"/></SubADBs></AnimDB>"#,
        )]);

        let clips = clips(&files, "a.adb").expect("clips");

        assert_eq!(clips, Clips::default());
    }

    #[test]
    fn two_databases_that_name_each_other_are_each_read_once() {
        let files = files(&[
            (
                "a.adb",
                r#"<AnimDB><SubADBs><SubADB File="b.adb"/></SubADBs><X><Animation name="one"/></X></AnimDB>"#,
            ),
            (
                "b.adb",
                r#"<AnimDB><SubADBs><SubADB File="a.adb"/></SubADBs><X><Animation name="two"/></X></AnimDB>"#,
            ),
        ]);

        let clips = clips(&files, "a.adb").expect("clips");

        assert_eq!(clips.names, ["one", "two"]);
        assert_eq!(clips.unread, []);
        assert_eq!(clips.truncated, Vec::<String>::new());
    }

    #[test]
    fn databases_beyond_the_limit_are_left_out_and_reported() {
        let nested: Vec<String> = (0..=MAX_DATABASES)
            .map(|at| format!(r#"<SubADB File="sub{at}.adb"/>"#))
            .collect();
        let root = format!("<AnimDB><SubADBs>{}</SubADBs></AnimDB>", nested.concat());
        let files = Files(vec![("a.adb".to_owned(), root)]);

        let clips = clips(&files, "a.adb").expect("clips");

        assert_eq!(
            clips.unread.len(),
            MAX_DATABASES - 1,
            "the root is read too"
        );
        assert_eq!(clips.truncated.len(), 1, "{:?}", clips.truncated);
        assert!(clips.truncated[0].contains("animation databases"));
    }

    #[test]
    fn names_beyond_the_limit_are_left_out_and_reported() {
        let names: Vec<String> = (0..=MAX_NAMES)
            .map(|at| format!(r#"<Animation name="clip{at}"/>"#))
            .collect();
        let root = format!("<AnimDB><X>{}</X></AnimDB>", names.concat());
        let files = Files(vec![("a.adb".to_owned(), root)]);

        let clips = clips(&files, "a.adb").expect("clips");

        assert_eq!(clips.names.len(), MAX_NAMES);
        assert_eq!(clips.truncated.len(), 1, "{:?}", clips.truncated);
        assert!(clips.truncated[0].contains("animation names"));
    }

    #[test]
    fn a_missing_or_unreadable_root_database_is_a_problem() {
        let files = files(&[("bad.adb", "not xml")]);

        assert_eq!(
            clips(&files, "none.adb").expect_err("missing"),
            DatabaseProblem::Missing
        );
        assert!(matches!(
            clips(&files, "bad.adb").expect_err("unreadable"),
            DatabaseProblem::Unreadable(_)
        ));
    }
}
