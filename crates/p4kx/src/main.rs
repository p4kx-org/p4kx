// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! `p4kx`, the command-line interface of P4KX.

mod actionmap;
mod armour;
mod armour_glb;
mod armour_loadout;
mod audio;
mod bnk;
mod bnk_objects;
mod bnk_objects_text;
mod catalog;
mod catalog_input;
mod catalog_json;
mod catalog_json_shops;
mod catalog_json_slugs;
mod catalog_size;
mod channels;
mod character_package;
mod chf;
mod chf_diff;
mod chf_glb;
mod chf_names;
mod chunks;
mod cli;
mod cli_actionmap;
mod cli_armour;
mod cli_audio;
mod cli_bnk;
mod cli_bnk_objects;
mod cli_catalog;
mod cli_channels;
mod cli_character;
mod cli_chf;
mod cli_chunks;
mod cli_conflict;
mod cli_convert;
mod cli_coverage;
mod cli_cryxml;
mod cli_dcb;
mod cli_dcb_fields;
mod cli_entity;
mod cli_extract;
mod cli_grep;
mod cli_hologram;
mod cli_interior;
mod cli_key;
mod cli_loc;
mod cli_model;
mod cli_output;
mod cli_prefab;
mod cli_ships;
mod cli_socpak;
mod cli_ui;
mod cli_verify;
mod cli_wwise;
mod compare;
mod compare_schema;
mod convert;
mod coverage;
mod coverage_json;
mod cryxml;
mod dataforges;
mod dcb;
mod dcb_export;
mod dcb_fields;
mod dcb_fields_out;
mod dcb_query;
mod dcb_refs;
mod dcb_refs_json;
mod dcb_refs_table;
mod dcb_tags;
mod dcb_tags_json;
mod dcb_tags_table;
mod diff;
mod entity;
mod entity_audio;
mod entity_glb;
mod entity_glb_body;
mod entity_glb_json;
mod entity_glb_placements;
mod entity_glb_report;
mod entity_ignored;
mod entity_json;
mod entity_outfit;
mod entity_package;
mod entity_root;
mod entity_vehicle;
mod exit;
mod extract;
mod extract_report;
mod failure_report;
mod filter_pair;
mod forge_input;
mod grep;
mod grep_json;
mod grep_table;
mod guard;
mod info;
mod interior_forge;
mod interior_glb;
mod interior_package;
mod issues;
mod json;
mod key;
mod layers_json;
mod list;
mod loc;
mod model;
mod model_animations;
mod model_bones;
mod model_glb;
mod model_info;
mod model_text;
mod no_match;
mod notice;
mod prefab;
mod progress;
mod query_file;
mod query_rows;
mod query_table;
mod renamed_report;
mod ships;
mod socpak;
mod test_build;
mod texture;
mod texture_image;
mod ui;
mod ui_extract;
mod ui_hologram;
mod ui_json;
mod ui_render;
mod ui_screen_render;
mod ui_screens;
mod verify;
mod wwise;
mod wwise_events;
mod wwise_extract;
mod wwise_json;
mod wwise_txtp;

use std::io::{self, Write};
use std::process::ExitCode;

use clap::Parser;

use crate::cli::{Cli, Command};

fn main() -> ExitCode {
    let cli = Cli::parse();
    let json = cli.json;
    guard::protect_installations();
    if !matches!(cli.command, Command::Key(_)) {
        guard::start_game_check();
    }
    if let Err(error) = key::init(&cli) {
        report(&error);
        return exit::for_error(&error);
    }
    let given = key::Given {
        key: cli.archive_key.clone(),
        file: cli.key_file.clone(),
    };
    let result = dispatch(cli.command, json, &given);
    match result {
        Ok(code) => code,
        Err(error) if is_broken_pipe(&error) => ExitCode::from(exit::SUCCESS),
        Err(error) => {
            report(&error);
            exit::for_error(&error)
        }
    }
}

fn report(error: &anyhow::Error) {
    let mut stderr = io::stderr().lock();
    let _ignored = writeln!(stderr, "error: {error:#}");
    if let Some(hint) = key::hint(error).or_else(|| no_match::no_match_hint(error)) {
        let _ignored = writeln!(stderr, "hint: {hint}");
    }
}

fn is_broken_pipe(error: &anyhow::Error) -> bool {
    error
        .downcast_ref::<io::Error>()
        .is_some_and(|error| error.kind() == io::ErrorKind::BrokenPipe)
}

/// Runs `command` and returns how it ended.
fn dispatch(command: Command, json: bool, given: &key::Given) -> anyhow::Result<ExitCode> {
    let success = |()| ExitCode::from(exit::SUCCESS);
    match command {
        Command::List(args) => list::run(&args, json).map(success),
        Command::Search(args) => list::search(&args, json).map(success),
        Command::Extract(args) => extract::run(&args, json),
        Command::Convert(args) => convert::run(&args, json),
        Command::Verify(args) => verify::run(&args, json),
        Command::Channels(args) => channels::run(&args, json).map(success),
        Command::Info(args) => info::run(&args, json).map(success),
        Command::Dcb(args) => dcb::run(&args, json),
        Command::Grep(args) => grep::run(&args, json),
        Command::Catalog(args) => catalog::run(&args, json),
        Command::Coverage(args) => coverage::run(&args, json),
        Command::Ships(args) => ships::run(&args, json),
        Command::Entity(args) => entity::run(&args, json),
        Command::Interior(args) => cli_interior::dispatch(args.action, json).map(success),
        Command::Prefab(args) => prefab::run(&args, json).map(success),
        Command::Armour(args) => armour::run(&args, json).map(success),
        Command::Character(args) => character_package::run(&args, json).map(success),
        Command::Loc(args) => loc::run(&args, json),
        Command::ActionMap(args) => actionmap::run(&args, json).map(success),
        Command::Compare(args) => Ok(compare::run(&args, json).unwrap_or_else(|error| {
            report(&error);
            exit::for_compare_error(&error)
        })),
        Command::CryXml(args) => cryxml::run(&args, json).map(success),
        Command::Texture(args) => texture::run(&args, json).map(success),
        Command::Audio(args) => audio::run(&args, json).map(success),
        Command::Bnk(args) => bnk::run(&args, json),
        Command::Ui(args) => ui::run(&args, json),
        Command::Socpak(args) => socpak::run(&args, json),
        Command::Chf(args) => chf::run(&args, json),
        Command::Chunks(args) => chunks::run(&args, json).map(success),
        Command::Wwise(args) => wwise::run(&args, json),
        Command::Model(args) => model::run(&args, json).map(success),
        Command::Key(args) => key::run(&args, given, json).map(success),
    }
}
