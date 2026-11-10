//! company.json and the Job files. Only the app writes these; the MCP server
//! sends requests instead, so two processes never write the same file.

use super::types::{Company, Job};
use anyhow::{Context, bail};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const COMPANY: &str = "company.json";
const COMPANY_TMP: &str = "company.json.tmp";
const JOBS: &str = "jobs";

/// JSON a person may have saved from a Windows editor, byte order mark and all.
pub(crate) fn parse_owned<T: serde::de::DeserializeOwned>(text: &str) -> serde_json::Result<T> {
    serde_json::from_str(text.strip_prefix('\u{FEFF}').unwrap_or(text))
}

/// The file isn't there. Windows says 2 for a missing file and 3 for a missing
/// folder on the way; access denied and the rest are not "missing".
pub(crate) fn missing(e: &io::Error) -> bool {
    e.kind() == io::ErrorKind::NotFound || matches!(e.raw_os_error(), Some(2 | 3))
}

pub fn company_root(home: &str) -> String {
    format!("{}\\.bytehive\\company", home.trim_end_matches(['\\', '/']))
}

/// Where this build keeps the company. A dev build never touches the owner's real
/// one unless it is told to, and only a dev build can be told.
pub fn pick_root(home: &str, dev: bool, root: Option<&str>) -> String {
    let real = company_root(home);
    if !dev {
        return real;
    }
    let set = root.map(clean_path).unwrap_or_default();
    let b = set.as_bytes();
    let drive = b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && b[2] == b'\\';
    let share = set.strip_prefix("\\\\").is_some_and(|s| s.split('\\').count() >= 2);
    // Before clean_path folds the dot of `\\.\` away.
    let raw = root.unwrap_or("").trim().replace('/', "\\");
    let device = raw.starts_with("\\\\?\\") || raw.starts_with("\\\\.\\");
    // The real company, or anything inside it, is never a sandbox, however it is spelled.
    if !(drive || share) || device || this_machine_share(&set) || inside(&set.to_lowercase(), &real.to_lowercase()) {
        return format!("{real}-dev");
    }
    set
}

fn inside(at: &str, dir: &str) -> bool {
    at == dir || at.starts_with(&format!("{dir}\\"))
}

/// `\\?\C:\...`, `\\.\C:\...`, `\\localhost\C$\...`: a spelling that can lead back to
/// this machine's own disks. A sandbox never needs one, so none is taken.
fn this_machine_share(set: &str) -> bool {
    let Some(rest) = set.strip_prefix("\\\\") else { return false };
    let mut parts = rest.split('\\');
    let host = parts.next().unwrap_or("").to_lowercase();
    let share = parts.next().unwrap_or("");
    let me = std::env::var("COMPUTERNAME").unwrap_or_default().to_lowercase();
    matches!(host.as_str(), "?" | "." | "localhost" | "127.0.0.1" | "::1" | "[::1]") || host == me || share.ends_with('$')
}

/// The folder a path really names once junctions, links and short names are
/// followed, as far as it exists, lowercased. The rest is joined on as written.
fn canonical(p: &Path) -> String {
    let mut rest = Vec::new();
    let mut at = p;
    loop {
        if let Ok(mut out) = fs::canonicalize(at) {
            out.extend(rest.iter().rev());
            return out.to_string_lossy().to_lowercase();
        }
        match (at.parent(), at.file_name()) {
            (Some(parent), Some(name)) => {
                rest.push(name.to_owned());
                at = parent;
            }
            _ => return p.to_string_lossy().to_lowercase(),
        }
    }
}

/// pick_root, then the same check on the disk: a junction or a short name that
/// leads into the real company is not a sandbox either.
pub fn resolve_root(home: &str, dev: bool, set: Option<&str>) -> PathBuf {
    let picked = pick_root(home, dev, set);
    let real = company_root(home);
    if picked != real && inside(&canonical(Path::new(&picked)), &canonical(Path::new(&real))) {
        return PathBuf::from(format!("{real}-dev"));
    }
    PathBuf::from(picked)
}

/// Backslashes, no trailing slash, and . and .. folded away.
fn clean_path(p: &str) -> String {
    let s = p.trim().replace('/', "\\");
    let mut out: Vec<&str> = Vec::new();
    for part in s.split('\\') {
        match part {
            "" | "." => {}
            ".." => {
                out.pop();
            }
            _ => out.push(part),
        }
    }
    let unc = if s.starts_with("\\\\") { "\\\\" } else { "" };
    format!("{unc}{}", out.join("\\"))
}

/// The company folder for this build: `BYTEHIVE_COMPANY_ROOT` in a dev build when
/// it names a sandbox, else `%USERPROFILE%\.bytehive\company` (`company-dev` in a
/// dev build). A test build counts as a dev build, `cargo test --release` included.
pub fn root() -> Option<PathBuf> {
    let home = std::env::var("USERPROFILE").ok()?;
    let set = std::env::var("BYTEHIVE_COMPANY_ROOT").ok();
    Some(resolve_root(&home, cfg!(debug_assertions) || cfg!(test), set.as_deref()))
}

/// Write through a temp file next to it, then swap it in whole: the MCP server
/// reads these at any moment, and a write cut short must not cost the file.
pub(crate) fn write_whole(dir: &Path, tmp: &str, name: &str, text: &str) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    fs::write(dir.join(tmp), text)?;
    fs::rename(dir.join(tmp), dir.join(name))
}

pub fn parse_company(text: &str) -> anyhow::Result<Company> {
    let c: Company = parse_owned(text).context("company.json has the wrong shape")?;
    if c.version != 1 {
        bail!("company.json has the wrong shape");
    }
    Ok(c)
}

/// An empty company when there is no file yet. Any other failure is an error, so
/// a company that could not be read is never started over.
pub fn load_company(root: &Path) -> anyhow::Result<Company> {
    match fs::read(root.join(COMPANY)) {
        Ok(bytes) => parse_company(&String::from_utf8(bytes).context("company.json is not UTF-8")?),
        Err(e) if missing(&e) => Ok(Company::empty()),
        Err(e) => Err(e).context("company.json could not be read"),
    }
}

pub fn save_company(root: &Path, c: &Company) -> anyhow::Result<()> {
    write_whole(root, COMPANY_TMP, COMPANY, &serde_json::to_string_pretty(c)?)?;
    Ok(())
}

pub fn save_job(root: &Path, job: &Job) -> anyhow::Result<()> {
    let name = format!("{}.json", job.id);
    write_whole(&root.join(JOBS), &format!("{name}.tmp"), &name, &serde_json::to_string_pretty(job)?)?;
    Ok(())
}

pub fn parse_job(text: &str) -> anyhow::Result<Job> {
    Ok(parse_owned(text).context("not a job")?)
}

/// Every Job, oldest first, and the names of the files that could not be read.
/// Those are left in place for the owner to mend (see job_file_alert).
pub fn load_jobs(root: &Path) -> anyhow::Result<(Vec<Job>, Vec<String>)> {
    let dir = root.join(JOBS);
    fs::create_dir_all(&dir)?;
    let mut entries: Vec<_> = fs::read_dir(&dir)?.filter_map(Result::ok).collect();
    entries.sort_by_key(|e| e.file_name().to_string_lossy().to_lowercase());
    let (mut jobs, mut unreadable) = (Vec::new(), Vec::new());
    for e in entries {
        let name = e.file_name().to_string_lossy().into_owned();
        if e.file_type().is_ok_and(|t| t.is_dir()) || !name.ends_with(".json") {
            continue;
        }
        match fs::read(e.path()).map_err(anyhow::Error::from).and_then(|b| parse_job(&String::from_utf8(b)?)) {
            Ok(job) => jobs.push(job),
            Err(_) => unreadable.push(name),
        }
    }
    jobs.sort_by_key(|j| j.created_at);
    Ok((jobs, unreadable))
}

/// The alert key and words for a Job file that could not be read.
pub fn job_file_alert(name: &str) -> (String, String) {
    (format!("job-file-{name}"), format!("The job file {name} could not be read. It was left in place: mend it and restart ByteHive."))
}

#[cfg(test)]
#[path = "store_tests.rs"]
mod tests;
