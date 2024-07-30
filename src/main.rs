use anyhow::anyhow;
use chrono::Local;
use colored::Colorize;
use inquire::{Confirm, MultiSelect, Text};
use once_cell::sync::Lazy;
use regex::Regex;
use std::ffi::OsStr;
use std::fmt::{Display, Formatter};
use std::fs::OpenOptions;
use std::io::{stdin, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Copy, Clone)]
enum IncludedFilesAction {
    MoveCopy,
    Export,
}

impl Display for IncludedFilesAction {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MoveCopy => f.write_str("Copy/Move"),
            Self::Export => f.write_str("Export paths"),
        }
    }
}

struct IncludedFile {
    inclusion_reason: &'static str,
    demo_path: PathBuf,
    events_json_path: PathBuf,
}

fn should_include_demo(content: &mut String, filter_ks_only: bool) -> (bool, &'static str) {
    // This regex matches event types
    // Example: '"name": "Bookmark",' => Bookmark
    static EVENT_TYPE_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r#""name":\s?+"(.*?)","#).unwrap());

    const EMPTY_EVENTS: &str = r#"{"events":[]}"#;

    // Remove whitespace
    content.retain(|c| !c.is_whitespace());

    // No events
    if content == EMPTY_EVENTS {
        return (true, "no events");
    }

    if filter_ks_only {
        for (_, [event_type]) in EVENT_TYPE_RE.captures_iter(content).map(|c| c.extract()) {
            if !event_type.eq_ignore_ascii_case("killstreak") {
                // This demo contains an event type that is not a killstreak and should not be included
                return (false, "has custom bookmark");
            }
        }

        return (true, "has only killstreak events");
    }

    (false, "has events")
}

fn get_output_name() -> String {
    static TIME: Lazy<String> = Lazy::new(|| {
        Local::now()
            .naive_local()
            .format("demclean-%Y-%m-%d-%H-%M-%S")
            .to_string()
    });
    
    TIME.clone()
}

fn action_move_copy(demos_dir: &Path, files: &mut [IncludedFile]) -> Result<(), anyhow::Error> {
    let default_dir = demos_dir.join(get_output_name());
    let output_dir = Text::new("Output directory")
        .with_default(default_dir.to_str().unwrap())
        .prompt()
        .unwrap();

    let output_dir = Path::new(&output_dir);

    if !output_dir.exists() {
        std::fs::create_dir(output_dir)?;
    }

    let copy = Confirm::new("Copy files?")
        .with_default(false)
        .with_help_message("Selecting no will move the included files to the output directory")
        .prompt()
        .unwrap();

    let verb = if copy { "Copied" } else { "Moved" };

    let file_op = if copy {
        |from: &PathBuf, to: &PathBuf| match std::fs::copy(from, to) {
            Ok(_) => Ok(()),
            Err(e) => Err(e),
        }
    } else {
        |from: &PathBuf, to: &PathBuf| std::fs::rename(from, to)
    };

    for file in files.iter_mut() {
        let demo_name = file.demo_path.file_name().unwrap();
        let new_path = output_dir.join(demo_name);
        file_op(&file.demo_path, &new_path)?;
        println!("{}", format!("\t{} {:?}", verb, demo_name).italic());
        if !copy {
            file.demo_path = new_path;
        }

        let events_json_name = file.events_json_path.file_name().unwrap();
        let new_path = output_dir.join(events_json_name);
        file_op(&file.events_json_path, &new_path)?;
        println!("{}", format!("\t{} {:?}", verb, events_json_name).italic());
        if !copy {
            file.events_json_path = new_path;
        }
    }

    println!(
        "{}",
        format!(
            "{} {} files to {}",
            verb,
            files.len() * 2,
            output_dir.to_str().unwrap()
        )
        .bright_green()
    );

    Ok(())
}

fn action_export(demos_dir: &Path, files: &Vec<IncludedFile>) -> Result<(), anyhow::Error> {
    let default_path = demos_dir.join(get_output_name()).with_extension("txt");
    let output_path = Text::new("Output file")
        .with_default(default_path.to_str().unwrap())
        .prompt()
        .unwrap();

    let include_json = Confirm::new("Export event json paths?")
        .with_default(true)
        .prompt()
        .unwrap();

    let output = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&output_path)?;

    let mut writer = BufWriter::new(output);

    fn path_to_bytes(path: &Path) -> Vec<u8> {
        path.to_str().unwrap().bytes().collect::<Vec<u8>>()
    }

    let mut count = 0;
    for file in files {
        writer.write_all(&path_to_bytes(&file.demo_path))?;
        writer.write_all(b"\n")?;
        if include_json {
            writer.write_all(&path_to_bytes(&file.events_json_path))?;
            writer.write_all(b"\n")?;
            count += 1;
        }
        count += 1;
    }

    println!(
        "{}",
        format!("Exported {} paths to {}", count, output_path).bright_green()
    );

    Ok(())
}

fn process() -> Result<(), anyhow::Error> {
    let demos_path = Text::new("Demos directory").prompt().unwrap();

    let demos_dir = Path::new(&demos_path);
    if !demos_dir.exists() {
        return Err(anyhow!("Directory {:?} does not exist", demos_dir));
    }

    println!(
        "\
Would you like to include demos that only contain Killstreak events?
This will exclude demos that contain custom bookmarks (added via 'ds_mark', etc.)\
"
    );

    let include_ks_only = Confirm::new("Include Killstreak only demos?")
        .with_default(false)
        .prompt()
        .unwrap_or(false);

    println!(
        "Demos containing only Killstreak events will {}be included.",
        if include_ks_only { "" } else { "not " }
    );

    let mut included_files = vec![];

    fn is_demo(ext: &Option<&OsStr>) -> bool {
        ext.and_then(OsStr::to_str)
            .map_or(false, |str| str == "dem")
    }

    for entry in std::fs::read_dir(demos_dir)?
        .map(|e| e.unwrap())
        .filter(|e| is_demo(&e.path().extension()))
    {
        let entry_path = entry.path();
        let file_name = entry_path.file_name().unwrap();
        let json_path = entry_path.with_extension("json");

        if !json_path.exists() {
            println!("Can't find json events file for demo {:?}", file_name);
            continue;
        }

        let (should_include, reason) = match std::fs::read_to_string(&json_path) {
            Ok(mut content) => should_include_demo(&mut content, include_ks_only),
            Err(e) => {
                eprintln!("Failed to read events json file {:?}: {:?}", json_path, e);
                (false, "failed to read json")
            }
        };

        if !should_include {
            println!("{} {:?}: {}", "Skipping".red(), file_name, reason);
            continue;
        }

        included_files.push(IncludedFile {
            inclusion_reason: reason,
            demo_path: entry_path.clone(),
            events_json_path: json_path.clone(),
        });
    }

    if included_files.is_empty() {
        println!("{}", "There are no included demos.".bright_red());
        return Ok(());
    }

    for file in &included_files {
        println!(
            "{} {:?}: {}",
            "Including".bright_green(),
            file.demo_path.file_name().unwrap(),
            file.inclusion_reason
        );
    }

    let actions = MultiSelect::new(
        &format!("Action for {} files", included_files.len()),
        vec![IncludedFilesAction::MoveCopy, IncludedFilesAction::Export],
    )
    .prompt()
    .unwrap();

    for action in actions {
        match action {
            IncludedFilesAction::MoveCopy => action_move_copy(demos_dir, &mut included_files)?,
            IncludedFilesAction::Export => action_export(demos_dir, &included_files)?,
        }
    }

    Ok(())
}

fn main() -> Result<(), anyhow::Error> {
    fn wait_key() {
        println!("\nPress any key to exit...");
        let _ = stdin().read(&mut []).unwrap();
    }

    let result = process();
    if let Err(e) = result {
        eprintln!("Error: {:?}", e);
        wait_key();
        return Err(e);
    }
    println!("{}", "Done.".bright_green());
    wait_key();
    Ok(())
}
