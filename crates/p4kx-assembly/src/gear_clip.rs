// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::AnimationChoice;
use p4kx_model::{Animation, Model};

use crate::cdf::{self, MAX_CDF_BYTES, is_character_definition};
use crate::markup::{self, Budget};
use crate::{ArchiveModels, ModelFailure, ModelProvider};

/// The most bytes of a skeleton's parameter file that are read.
pub(crate) const MAX_CHRPARAMS_BYTES: u64 = 1 << 20;

/// The most folders, and the most databases, of a parameter file that are kept; every folder costs
/// a scan of the Archive's entries.
pub(crate) const MAX_PARAM_SOURCES: usize = 64;

/// What a skeleton's parameter file (`.chrparams`) lists: the folders and databases its animations
/// are stored in, in file order, and the animations by name and file.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct ChrParams {
    /// The `#filepath` folders, at most [`MAX_PARAM_SOURCES`]; past it the last slot holds the
    /// latest, so the last folder is always the file's last.
    pub(crate) folders: Vec<String>,
    /// The `$TracksDatabase` files, capped like `folders`.
    pub(crate) databases: Vec<String>,
    pub(crate) clips: Vec<(String, String)>,
    /// Whether folders or databases were dropped for the cap.
    pub(crate) capped: bool,
}

/// Adds `path` to `list`, which holds at most [`MAX_PARAM_SOURCES`]; returns whether one was
/// dropped for it.
fn keep_last(list: &mut Vec<String>, path: String) -> bool {
    if list.len() < MAX_PARAM_SOURCES {
        list.push(path);
        false
    } else {
        if let Some(last) = list.last_mut() {
            *last = path;
        }
        true
    }
}

/// Reads the parameter file in `bytes`.
///
/// # Errors
///
/// Returns why the document is no parameter file.
pub(crate) fn read_params(bytes: &[u8]) -> Result<ChrParams, String> {
    let mut budget = Budget::new(bytes.len());
    let root = markup::parse(bytes, &mut budget)?;
    if !root.name.eq_ignore_ascii_case("Params") {
        return Err(format!("the root element is {}, not Params", root.name));
    }
    let mut params = ChrParams {
        folders: Vec::new(),
        databases: Vec::new(),
        clips: Vec::new(),
        capped: false,
    };
    for list in root.children_named("AnimationList") {
        for clip in list.children_named("Animation") {
            let (Some(name), Some(file)) = (clip.attribute("name"), clip.attribute("path")) else {
                continue;
            };
            let dropped = match name {
                "#filepath" => keep_last(&mut params.folders, file.replace('\\', "/")),
                "$TracksDatabase" => keep_last(&mut params.databases, file.replace('\\', "/")),
                _ => {
                    params.clips.push((name.to_owned(), file.to_owned()));
                    false
                }
            };
            params.capped |= dropped;
        }
    }
    Ok(params)
}

impl ChrParams {
    /// Returns the last `#filepath` folder and the last `$TracksDatabase`, the only ones a landing
    /// gear's clip is looked for in.
    pub(crate) fn last_sources(&self) -> (Option<&str>, Option<&str>) {
        (
            self.folders.last().map(String::as_str),
            self.databases.last().map(String::as_str),
        )
    }

    /// Returns the file of the clip that deploys the gear: the first whose name has `extend`,
    /// else the first with `deploy`, but never one that also names a retraction or compression.
    pub(crate) fn deploy_clip(&self) -> Option<&str> {
        ["extend", "deploy"].iter().find_map(|word| {
            self.clips.iter().find_map(|(name, file)| {
                let lower = name.to_ascii_lowercase();
                (lower.contains(word) && !lower.contains("retract") && !lower.contains("compress"))
                    .then_some(file.as_str())
            })
        })
    }
}

/// Returns `path` with the extension `.chrparams`.
pub(crate) fn params_path(skeleton: &str) -> String {
    let stem = skeleton.rsplit_once('.').map_or(skeleton, |(stem, _)| stem);
    format!("{stem}.chrparams")
}

impl ArchiveModels<'_> {
    pub(crate) fn pose_gear(
        &self,
        path: &str,
        model: &Model,
    ) -> Result<Option<Model>, ModelFailure> {
        let Some(entry) = self.entry_of(path) else {
            return Ok(None);
        };
        if !is_character_definition(&entry) {
            return Ok(None);
        }
        let unreadable = |reason: String| ModelFailure::Unreadable { reason };
        let bytes = self.file(&entry, MAX_CDF_BYTES)?;
        let character = cdf::read(&bytes).map_err(unreadable)?;
        let Some(skeleton) = character.skeleton else {
            return Ok(None);
        };
        let params = match self.file(&params_path(&skeleton), MAX_CHRPARAMS_BYTES) {
            Ok(bytes) => read_params(&bytes).map_err(unreadable)?,
            Err(ModelFailure::Missing) => return Ok(None),
            Err(other) => return Err(other),
        };
        let Some(clip) = params.deploy_clip() else {
            return Ok(None);
        };
        let animation = self.read_clip(&params, clip)?;
        crate::gear_pose::posed(model, &animation)
            .map(Some)
            .map_err(|failure| unreadable(failure.to_string()))
    }

    fn read_clip(&self, params: &ChrParams, clip: &str) -> Result<Animation, ModelFailure> {
        let (folder, database) = params.last_sources();
        let extra: Vec<String> = database
            .into_iter()
            .filter_map(|database| self.entry_of(database))
            .chain(folder.into_iter().filter_map(|folder| {
                self.entry_of(&format!("{}/{clip}", folder.trim_end_matches('/')))
            }))
            .collect();
        if extra.is_empty() {
            return Err(ModelFailure::Unreadable {
                reason: "the animation database and the clip are not in the Archive".to_owned(),
            });
        }
        let choice = AnimationChoice::Named(vec![clip.to_owned()]);
        let set =
            self.animations_of(&extra, &choice)
                .map_err(|error| ModelFailure::Unreadable {
                    reason: error.to_string(),
                })?;
        set.animations
            .into_iter()
            .find(|animation| !animation.additive)
            .ok_or_else(|| ModelFailure::Unreadable {
                reason: format!("the clip {clip} is not in the animation database"),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::{MAX_PARAM_SOURCES, params_path, read_params};

    const PARAMS: &str = r##"<Params><AnimationList>
        <Animation name="#filepath" path="Animations\Ships\Gear"/>
        <Animation name="$TracksDatabase" path="Animations/Ships/Gear.dba"/>
        <Animation name="gear_compress" path="front_compress.caf"/>
        <Animation name="gear_extend" path="front_extend.caf"/>
        <Animation name="gear_retract" path="front_retract.caf"/>
        </AnimationList></Params>"##;

    #[test]
    fn the_extending_clip_is_the_one_that_deploys() {
        let params = read_params(PARAMS.as_bytes()).expect("params");
        assert_eq!(params.folders, ["Animations/Ships/Gear"]);
        assert_eq!(params.databases, ["Animations/Ships/Gear.dba"]);
        assert_eq!(params.deploy_clip(), Some("front_extend.caf"));
    }

    #[test]
    fn deploy_is_taken_when_nothing_extends_and_compress_or_retract_never() {
        let text = r#"<Params><AnimationList>
            <Animation name="gear_retract" path="a_retract.caf"/>
            <Animation name="gear_compress" path="a_compress.caf"/>
            <Animation name="gear_deploy" path="a_deploy.caf"/>
            </AnimationList></Params>"#;
        assert_eq!(
            read_params(text.as_bytes()).expect("params").deploy_clip(),
            Some("a_deploy.caf")
        );
        let none = r#"<Params><AnimationList>
            <Animation name="gear_retract" path="a_retract.caf"/>
            </AnimationList></Params>"#;
        assert_eq!(
            read_params(none.as_bytes()).expect("params").deploy_clip(),
            None
        );
    }

    #[test]
    fn a_document_that_is_no_parameter_file_is_refused() {
        assert!(read_params(b"<Other/>").is_err());
        assert!(read_params(b"not xml").is_err());
    }

    const SEVERAL: &str = r##"<Params><AnimationList>
        <Animation name="#filepath" path="Animations\First"/>
        <Animation name="$TracksDatabase" path="Animations/First.dba"/>
        <Animation name="#filepath" path="Animations/Second"/>
        <Animation name="$TracksDatabase" path="Animations/Second.dba"/>
        </AnimationList></Params>"##;

    #[test]
    fn every_folder_and_database_is_kept_in_file_order() {
        let params = read_params(SEVERAL.as_bytes()).expect("params");

        assert_eq!(params.folders, ["Animations/First", "Animations/Second"]);
        assert_eq!(
            params.databases,
            ["Animations/First.dba", "Animations/Second.dba"]
        );
        assert!(!params.capped);
    }

    #[test]
    fn a_landing_gear_takes_the_last_folder_and_database() {
        let params = read_params(SEVERAL.as_bytes()).expect("params");

        assert_eq!(
            params.last_sources(),
            (Some("Animations/Second"), Some("Animations/Second.dba"))
        );
    }

    #[test]
    fn folders_beyond_the_cap_are_dropped_but_the_last_stays_last() {
        let list: Vec<String> = (0..=MAX_PARAM_SOURCES)
            .map(|at| format!(r##"<Animation name="#filepath" path="Animations/f{at}"/>"##))
            .collect();
        let text = format!(
            "<Params><AnimationList>{}</AnimationList></Params>",
            list.concat()
        );

        let params = read_params(text.as_bytes()).expect("params");

        assert_eq!(params.folders.len(), MAX_PARAM_SOURCES);
        assert!(params.capped);
        assert_eq!(
            params.last_sources().0,
            Some(format!("Animations/f{MAX_PARAM_SOURCES}").as_str())
        );
    }

    #[test]
    fn the_parameter_file_sits_beside_the_skeleton() {
        assert_eq!(
            params_path("Objects/a/Gear_CHR.chr"),
            "Objects/a/Gear_CHR.chrparams"
        );
    }
}
