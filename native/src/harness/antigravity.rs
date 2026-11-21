//! Antigravity (agy): the laws as a rules file in the run folder.
//!
//! agy reads every `.agents/rules/*.md` of its workspace as standing rules
//! (tested 2026-11-13); its --settings takes no rules, hooks or MCP servers
//! (measured: "unknown setting"). The file is ByteHive's, so git's exclude file
//! keeps it out of `git status`, as it does `.bytehive/`.

use std::path::Path;

use super::Plan;

pub const RULES: &str = ".agents/rules/bytehive.md";

pub fn plan(run_dir: &str, laws: &str) -> Plan {
    let text = format!("---\ntrigger: always_on\n---\n\n# ByteHive\n\n{laws}\n");
    Plan { args: Vec::new(), file: Some((Path::new(run_dir).join(RULES), text)), exclude: Some(RULES.to_string()), ..Default::default() }
}
