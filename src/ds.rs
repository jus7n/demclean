use crate::{util, IncludedDemo};
use colored::Colorize;
use once_cell::sync::Lazy;
use regex::Regex;
use std::path::Path;

// This regex matches event types
// Example: '"name": "Bookmark",' => Bookmark
static EVENT_EXTRACT_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r#""name":\s?+"(.*?)","#).unwrap());

const EMPTY_EVENTS: &str = r#"{"events":[]}"#;

fn should_include_demo(content: &mut String, filter_ks_only: bool) -> (bool, &'static str) {
    // Remove whitespace
    content.retain(|c| !c.is_whitespace());

    // No events
    if content == EMPTY_EVENTS {
        return (true, "no events");
    }

    if filter_ks_only {
        for (_, [event_type]) in EVENT_EXTRACT_RE
            .captures_iter(content)
            .map(|ref c| c.extract())
        {
            if !event_type.eq_ignore_ascii_case("killstreak") {
                // This demo contains an event type that is not a killstreak and should not be included
                return (false, "has custom bookmark");
            }
        }

        return (true, "has only killstreak events");
    }

    (false, "has events")
}

pub fn collect_ds_demos(
    demos_dir: &Path,
    include_ks_only: bool,
    included_files: &mut Vec<IncludedDemo>,
) -> Result<(), anyhow::Error> {
    for entry in std::fs::read_dir(demos_dir)?
        .map(|e| e.unwrap())
        .filter(|e| util::is_demo(&e.path().extension()))
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

        included_files.push(IncludedDemo {
            inclusion_reason: reason,
            demo_path: entry_path,
            events_json_path: Some(json_path.clone()),
            id: "demosupport",
        });
    }

    Ok(())
}
