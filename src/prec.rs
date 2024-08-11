use crate::{util, IncludedDemo};
use colored::Colorize;
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

// This regex matches event types and demo names within the PREC event file
// Example: [2023/11/27/ 22:01] Kill Streak:5 ("20231127_2152_cp_altitude_RED_BLU" at 32900) => Kill Streak:5, 20231127_2152_cp_altitude_RED_BLU
static EVENT_EXTRACT_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"\[[\d/\s:]+]\s?(.+)\s\("(.+)"\s?at"#).unwrap());

// This regex matches the PREC 'Kill Streak:#' event type
static KS_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r#"Kill\sStreak:\d+"#).unwrap());

const PREC_KS_FILE: &str = "KillStreaks.txt";

fn find_prec_ks_file(demos_dir: &Path) -> Option<PathBuf> {
    let search = [
        Some(demos_dir.join(PREC_KS_FILE)),
        demos_dir.parent().map(|par| par.join(PREC_KS_FILE)),
    ];

    search.into_iter().flatten().find(|path| path.exists())
}

fn should_include_demo(
    events: Option<&HashSet<&str>>,
    filter_ks_only: bool,
) -> (bool, &'static str) {
    if events.is_none() {
        return (true, "no events");
    }

    if filter_ks_only {
        for event_name in events.unwrap() {
            if !KS_RE.is_match(event_name) {
                // This demo contains an event type that is not a killstreak and should not be included
                return (false, "has custom bookmark");
            }
        }
        return (true, "has only killstreak events");
    }

    (false, "has events")
}

pub fn collect_prec_demos(
    demos_dir: &Path,
    include_ks_only: bool,
    included_files: &mut Vec<IncludedDemo>,
) -> Result<(), anyhow::Error> {
    let ks_file = match find_prec_ks_file(demos_dir) {
        Some(ks_file) => ks_file,
        None => {
            println!(
                "{}",
                format!(
                    "Failed to find PREC event log file. Ensure '{}' is in the selected demos directory or its parent directory.",
                    PREC_KS_FILE
                ).bright_red()
            );
            return Ok(());
        }
    };

    let demos_dir = ks_file.parent().unwrap();

    let event_file_content = std::fs::read_to_string(&ks_file)?;

    let mut event_map = HashMap::new();

    // Collect all referenced valid demos along with their events
    for (_, [event_type, demo_file_name]) in EVENT_EXTRACT_RE
        .captures_iter(&event_file_content)
        .map(|ref c| c.extract())
    {
        let demo_file_name = demo_file_name.to_lowercase() + ".dem";

        let demo_path = demos_dir.join(&demo_file_name);
        if !demo_path.exists() {
            continue;
        }

        let events = event_map.entry(demo_path).or_insert(HashSet::new());
        events.insert(event_type);
    }

    for entry in std::fs::read_dir(demos_dir)?
        .map(|e| e.unwrap())
        .filter(|e| util::is_demo(&e.path().extension()))
    {
        let entry_path = entry.path();
        let file_name = entry_path.file_name().unwrap();

        let (should_include, reason) =
            should_include_demo(event_map.get(&entry_path), include_ks_only);

        if !should_include {
            println!("{} {:?}: {}", "Skipping".red(), file_name, reason);
            continue;
        }

        included_files.push(IncludedDemo {
            inclusion_reason: reason,
            demo_path: entry_path,
            events_json_path: None,
            id: "prec",
        });
    }

    Ok(())
}
