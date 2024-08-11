mod ds;
mod prec;
mod util;

use anyhow::anyhow;
use colored::Colorize;
use inquire::{Confirm, MultiSelect, Select, Text};
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

pub struct IncludedDemo {
    inclusion_reason: &'static str,
    demo_path: PathBuf,
    // DemoSupport specific
    events_json_path: Option<PathBuf>,
    // 'demosupport', 'prec', etc.
    id: &'static str,
}

impl IncludedDemo {
    pub fn move_to(&mut self, copy: bool, output_dir: &Path) -> Result<(), anyhow::Error> {
        let output_dir = output_dir.join(self.id);
        if !output_dir.exists() {
            std::fs::create_dir(&output_dir)?;
        }

        let file_op = |from: &PathBuf, to: &PathBuf| match copy {
            true => std::fs::copy(from, to).map(|_| ()),
            false => std::fs::rename(from, to),
        };

        let move_file = |path: &mut PathBuf| -> Result<(), anyhow::Error> {
            let file_name = path.file_name().unwrap();
            let new_path = output_dir.join(file_name);

            file_op(path, &new_path)?;

            let verb = if copy { "Copied" } else { "Moved" };
            println!("{}", format!("\t{} {:?}", verb, file_name).italic());

            if !copy {
                *path = new_path;
            }

            Ok(())
        };

        move_file(&mut self.demo_path)?;
        if let Some(events_json_path) = &mut self.events_json_path {
            move_file(events_json_path)?;
        }

        Ok(())
    }
}

fn action_move_copy(demos_dir: &Path, files: &mut [IncludedDemo]) -> Result<(), anyhow::Error> {
    let default_dir = demos_dir.join(util::get_output_name());
    let output_dir = Text::new("Output directory?")
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

    for file in files.iter_mut() {
        file.move_to(copy, output_dir)?;
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

fn action_export(demos_dir: &Path, files: &Vec<IncludedDemo>) -> Result<(), anyhow::Error> {
    let default_path = demos_dir
        .join(util::get_output_name())
        .with_extension("txt");
    let output_path = Text::new("Output file?")
        .with_default(default_path.to_str().unwrap())
        .prompt()
        .unwrap();

    let include_json = Confirm::new("Export event json paths?")
        .with_help_message("Only applicable to DemoSupport demos")
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
            if let Some(events_json_path) = &file.events_json_path {
                writer.write_all(&path_to_bytes(events_json_path))?;
                writer.write_all(b"\n")?;
                count += 1;
            }
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
    let demos_path = Text::new("Demos directory?").prompt().unwrap();

    let demos_dir = Path::new(&demos_path);
    if !demos_dir.exists() {
        return Err(anyhow!("Directory {:?} does not exist", demos_dir));
    }

    let demo_mode = Select::new("Demo search mode?", vec!["DemoSupport", "PREC"])
        .prompt()
        .unwrap();

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

    let mut included_demos = vec![];

    if demo_mode == "PREC" {
        println!("{}", "Searching for PREC demos...".bright_green());
        prec::collect_prec_demos(demos_dir, include_ks_only, &mut included_demos)?;
    } else {
        println!("{}", "Searching for DemoSupport demos...".bright_green());
        ds::collect_ds_demos(demos_dir, include_ks_only, &mut included_demos)?;
    }

    if included_demos.is_empty() {
        println!("{}", "There are no included demos.".bright_red());
        return Ok(());
    }

    for file in &included_demos {
        println!(
            "{} {:?}: {}",
            "Including".bright_green(),
            file.demo_path.file_name().unwrap(),
            file.inclusion_reason
        );
    }

    let actions = MultiSelect::new(
        &format!("Action for {} files", included_demos.len()),
        vec![IncludedFilesAction::MoveCopy, IncludedFilesAction::Export],
    )
    .prompt()
    .unwrap();

    for action in actions {
        match action {
            IncludedFilesAction::MoveCopy => action_move_copy(demos_dir, &mut included_demos)?,
            IncludedFilesAction::Export => action_export(demos_dir, &included_demos)?,
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
