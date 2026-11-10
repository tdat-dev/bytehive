//! Bringing the owner's fleet and settings over from the ByteHive app before
//! this one (0.14.2 and later write `~/.bytehive/export/web-state.json`,
//! export.ts there). Read once: a marker in the data folder says it was done.
//! Agents arrive stopped, like after any restart, and Resume carries on their
//! own conversations. Company members stay with that app.

use std::path::PathBuf;

use serde::Deserialize;
use serde_json::Value;

use crate::fleet::{Race, Spec, now_ms};
use crate::presets::Prefs;

/// The export's shape this reads (EXPORT_FORMAT in export.ts).
const FORMAT: u64 = 1;
const MARKER: &str = "imported.json";

pub fn export_file() -> Option<PathBuf> {
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"));
    let sandbox = std::env::var_os("BYTEHIVE_NATIVE_DIR").map(PathBuf::from);
    export_path(sandbox.as_deref(), home.as_deref())
}

/// A run in a sandbox (BYTEHIVE_NATIVE_DIR) reads an export only from inside it,
/// so a dev run never takes the owner's projects and agents.
fn export_path(sandbox: Option<&std::path::Path>, home: Option<&std::ffi::OsStr>) -> Option<PathBuf> {
    match sandbox {
        Some(dir) => Some(dir.join("export").join("web-state.json")),
        None => Some(PathBuf::from(home?).join(".bytehive").join("export").join("web-state.json")),
    }
}

/// One agent as the old app saved it (AgentSpec in panetypes.ts).
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OldAgent {
    program: String,
    #[serde(default)]
    args: Vec<String>,
    cwd: Option<String>,
    name: String,
    worktree: Option<String>,
    branch: Option<String>,
    title: Option<String>,
    race: Option<OldRace>,
    session_id: Option<String>,
    resume_in: Option<String>,
    person_id: Option<String>,
}

#[derive(Deserialize)]
struct OldRace {
    id: String,
    n: usize,
    of: usize,
}

#[derive(Deserialize)]
struct OldProject {
    dir: Option<String>,
    #[serde(default)]
    agents: Vec<OldAgent>,
}

/// What the export holds that this app keeps.
#[derive(Default)]
pub struct Found {
    pub specs: Vec<Spec>,
    pub recents: Vec<String>,
    pub last_cli: Option<String>,
    pub last_custom: Option<String>,
    pub skip_perms: Option<bool>,
    pub default_cli: Option<String>,
    pub default_count: Option<usize>,
    pub job_delay: Option<u64>,
    pub worktree: Option<bool>,
    pub restore: Option<bool>,
    pub notify_needs: Option<bool>,
    pub notify_done: Option<bool>,
    pub auto_handover: Option<bool>,
    pub auto_resume: Option<bool>,
    pub limit_notify: Option<bool>,
    pub jump_to_needs: Option<bool>,
    pub review_on_done: Option<bool>,
    pub split_max: Option<usize>,
    pub director_first: Option<bool>,
    pub font_size: Option<u32>,
    pub hide_to_tray: bool,
    pub tips: Option<bool>,
    pub tips_seen: Option<Vec<String>>,
    pub exported_at: u64,
}

/// Read an export. Every entry is optional: a missing or broken one is
/// skipped, the rest still comes over.
pub fn read(json: &str) -> Result<Found, String> {
    let root: Value = serde_json::from_str(json).map_err(|e| format!("the export is not JSON: {e}"))?;
    if root.get("format").and_then(Value::as_u64) != Some(FORMAT) {
        return Err("the export is from a version this app does not read".to_string());
    }
    let storage = root.get("storage").and_then(Value::as_object).ok_or("the export has no storage")?;
    let raw = |key: &str| storage.get(key).and_then(Value::as_str);
    let parsed = |key: &str| raw(key).and_then(|s| serde_json::from_str::<Value>(s).ok());
    let mut found = Found { exported_at: root.get("exportedAt").and_then(Value::as_u64).unwrap_or(0), ..Found::default() };

    let projects: Vec<OldProject> = raw("maestro.session").and_then(|s| serde_json::from_str(s).ok()).unwrap_or_default();
    for project in projects {
        for a in project.agents {
            if a.person_id.is_some() || a.program.trim().is_empty() {
                continue;
            }
            let Some(cwd) = a.cwd.clone().or_else(|| project.dir.clone()).filter(|d| !d.is_empty()) else { continue };
            found.specs.push(Spec {
                id: uuid::Uuid::new_v4().to_string(),
                name: a.name,
                cli: a.program,
                args: a.args,
                cwd,
                session_id: a.session_id.filter(|s| !s.is_empty()),
                started_ms: now_ms(),
                title: a.title.filter(|t| !t.is_empty()),
                race: a.race.map(|r| Race { id: r.id, n: r.n, of: r.of }),
                // A conversation carried on in another folder runs there.
                worktree: a.worktree.or(a.resume_in),
                branch: a.branch,
                merged_into: None,
                role: None,
                person: None,
            });
        }
    }
    if let Some(Value::Array(list)) = parsed("maestro.recent") {
        found.recents = list.into_iter().filter_map(|v| v.as_str().map(str::to_string)).filter(|d| !d.is_empty()).collect();
    }
    found.last_cli = raw("maestro.inbox.lastCli").filter(|s| !s.is_empty()).map(str::to_string);
    found.last_custom = raw("maestro.inbox.lastCustom").filter(|s| !s.is_empty()).map(str::to_string);
    found.skip_perms = parsed("maestro.crew").and_then(|c| c.get("skipPerms").and_then(Value::as_bool));
    if let Some(p) = parsed("maestro.prefs") {
        found.default_cli = p.get("defaultCli").and_then(Value::as_str).map(str::to_string);
        found.default_count = p.get("defaultCount").and_then(Value::as_u64).map(|n| n as usize);
        found.job_delay = p.get("jobDelay").and_then(Value::as_f64).map(|n| n.round() as u64);
        found.worktree = p.get("worktree").and_then(Value::as_bool);
        found.restore = p.get("restore").and_then(Value::as_bool);
        found.notify_needs = p.get("notifyNeeds").and_then(Value::as_bool);
        found.notify_done = p.get("notifyDone").and_then(Value::as_bool);
        found.auto_handover = p.get("autoHandover").and_then(Value::as_bool);
        found.auto_resume = p.get("autoResume").and_then(Value::as_bool);
        found.limit_notify = p.get("limitNotify").and_then(Value::as_bool);
        found.jump_to_needs = p.get("jumpToNeeds").and_then(Value::as_bool);
        found.review_on_done = p.get("reviewOnDone").and_then(Value::as_bool);
        found.split_max = p.get("splitMax").and_then(Value::as_u64).map(|n| n as usize);
        found.director_first = p.get("directorFirst").and_then(Value::as_bool);
        found.tips = p.get("tips").and_then(Value::as_bool);
    }
    if let Some(Value::Array(list)) = parsed("maestro.tips.seen") {
        found.tips_seen = Some(list.into_iter().filter_map(|v| v.as_str().map(str::to_string)).collect());
    }
    found.font_size = raw("maestro.termFontSize").and_then(|s| s.trim().parse::<f64>().ok()).map(|n| n.round() as u32);
    found.hide_to_tray = raw("maestro.hideToTray") == Some("1");
    Ok(found)
}

impl Found {
    /// The old app's settings win once: it was the one in daily use.
    pub fn apply_prefs(&self, prefs: &mut Prefs) {
        let mut recents = self.recents.clone();
        for d in &prefs.recents {
            if !recents.iter().any(|r| crate::presets::same_folder(r, d)) {
                recents.push(d.clone());
            }
        }
        recents.truncate(crate::presets::RECENTS);
        prefs.recents = recents;
        if let Some(v) = &self.last_cli {
            prefs.last_cli = Some(v.clone());
        }
        if let Some(v) = &self.last_custom {
            prefs.last_custom = v.clone();
        }
        if let Some(v) = self.skip_perms {
            prefs.skip_perms = v;
        }
        if let Some(v) = &self.default_cli {
            prefs.default_cli = v.clone();
        }
        if let Some(v) = self.default_count {
            prefs.default_count = v;
        }
        if let Some(v) = self.job_delay {
            prefs.job_delay = v;
        }
        if let Some(v) = self.worktree {
            prefs.worktree = v;
        }
        if let Some(v) = self.restore {
            prefs.restore = v;
        }
        for (from, to) in [(self.notify_needs, &mut prefs.notify_needs), (self.notify_done, &mut prefs.notify_done), (self.auto_handover, &mut prefs.auto_handover), (self.auto_resume, &mut prefs.auto_resume), (self.limit_notify, &mut prefs.limit_notify), (self.jump_to_needs, &mut prefs.jump_to_needs), (self.review_on_done, &mut prefs.review_on_done), (self.director_first, &mut prefs.director_first), (self.tips, &mut prefs.tips)] {
            if let Some(v) = from {
                *to = v;
            }
        }
        if let Some(v) = self.font_size {
            prefs.font_size = v;
        }
        if let Some(v) = self.split_max {
            prefs.split_max = v;
        }
        prefs.hide_to_tray = self.hide_to_tray;
        if let Some(seen) = &self.tips_seen {
            prefs.tips_seen = seen.clone();
        }
        prefs.clamp();
    }
}

/// The export, when there is one this data folder has not taken yet: its
/// settings go into `prefs` (saved), its agents come back to be added. The
/// marker is written once they are, by `finish`.
pub fn take(prefs: &mut Prefs) -> Option<Found> {
    let dir = crate::fleet::data_dir()?;
    if done(&dir) {
        return None;
    }
    let json = std::fs::read_to_string(export_file()?).ok()?;
    let found = read(&json).ok()?;
    found.apply_prefs(prefs);
    prefs.save();
    Some(found)
}

pub fn finish(found: &Found, agents: usize) {
    if let Some(dir) = crate::fleet::data_dir() {
        mark_done(&dir, found.exported_at, agents);
    }
}

/// Whether this data folder has taken an export before.
pub fn done(dir: &std::path::Path) -> bool {
    dir.join(MARKER).exists()
}

pub fn mark_done(dir: &std::path::Path, exported_at: u64, agents: usize) {
    let body = serde_json::json!({ "exportedAt": exported_at, "importedAt": now_ms(), "agents": agents });
    let _ = std::fs::create_dir_all(dir);
    let _ = std::fs::write(dir.join(MARKER), body.to_string());
}

#[cfg(test)]
mod tests {
    use super::read;

    #[test]
    fn a_sandbox_reads_the_export_only_from_inside_itself() {
        use std::path::{Path, PathBuf};
        let home = std::ffi::OsStr::new("C:\\Users\\me");
        assert_eq!(super::export_path(Some(Path::new("D:\\sb\\native")), Some(home)), Some(PathBuf::from("D:\\sb\\native\\export\\web-state.json")));
        assert_eq!(super::export_path(None, Some(home)), Some(PathBuf::from("C:\\Users\\me\\.bytehive\\export\\web-state.json")));
        assert_eq!(super::export_path(None, None), None);
    }

    #[test]
    fn brings_agents_and_settings_over() {
        let session = r##"[{"name":"bytehive","dir":"D:\\bytehive","agents":[
            {"program":"claude","args":[],"cwd":"D:\\bytehive","name":"Ana","badge":"claude","color":"#d97757","mono":"C","sessionId":"5f440d65-c9e9-48a5-afc1-b57b7cb538c1","title":"Fix it","worktree":"D:\\.bytehive-worktrees\\bytehive-1\\ana","branch":"bytehive/ana-1"},
            {"program":"codex","args":["--yolo"],"cwd":null,"name":"Bob","badge":"codex","color":"#fff","mono":"X"},
            {"program":"claude","args":[],"cwd":"D:\\bytehive","name":"Founder","badge":"claude","color":"#fff","mono":"F","personId":"founder"}]}]"##;
        let storage = serde_json::json!({
            "maestro.session": session,
            "maestro.prefs": r#"{"defaultCli":"codex","defaultCount":2,"jobDelay":6,"worktree":true,"restore":true,"splitMax":4}"#,
            "maestro.recent": r#"["D:\\bytehive","D:\\app"]"#,
            "maestro.inbox.lastCli": "custom:aider --yes",
            "maestro.crew": r#"{"skipPerms":true}"#,
            "maestro.termFontSize": "17",
            "maestro.hideToTray": "1"
        });
        let json = serde_json::json!({ "format": 1, "exportedAt": 5, "storage": storage }).to_string();
        let f = read(&json).unwrap();
        assert_eq!(f.specs.len(), 2, "the company's Founder stays behind");
        assert_eq!(f.specs[0].session_id.as_deref(), Some("5f440d65-c9e9-48a5-afc1-b57b7cb538c1"));
        assert_eq!(f.specs[0].run_dir(), r"D:\.bytehive-worktrees\bytehive-1\ana");
        assert_eq!(f.specs[1].cwd, r"D:\bytehive", "no folder of its own: the project's");
        assert_eq!(f.specs[1].args, vec!["--yolo".to_string()]);
        let mut p = crate::presets::Prefs::default();
        p.recents = vec![r"D:\other".to_string()];
        f.apply_prefs(&mut p);
        assert_eq!((p.default_cli.as_str(), p.default_count, p.job_delay, p.worktree, p.font_size, p.hide_to_tray, p.skip_perms), ("codex", 2, 6, true, 17, true, true));
        assert_eq!(p.recents, vec![r"D:\bytehive".to_string(), r"D:\app".to_string(), r"D:\other".to_string()]);
        assert_eq!(p.last_cli.as_deref(), Some("custom:aider --yes"));
        assert_eq!(p.split_max, 4);
    }

    #[test]
    fn refuses_another_format() {
        assert!(read(r#"{"format":2,"storage":{}}"#).is_err());
        assert!(read("not json").is_err());
    }
}
