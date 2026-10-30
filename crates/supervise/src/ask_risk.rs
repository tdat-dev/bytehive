//! How much care an approval wants: destructive or privileged runs and edits
//! to sensitive files are high, plain reads and questions are low.

use std::sync::OnceLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::ask::AskKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AskRisk {
    Low,
    Mid,
    High,
}

/// Destructive, privileged, or published-to-the-world commands.
fn risky_run_res() -> &'static [Regex] {
    static RES: OnceLock<Vec<Regex>> = OnceLock::new();
    RES.get_or_init(|| {
        [
            r"\brm\s+-[a-z]*r[a-z]*f",
            r"(?i)\brd\s+\/s",
            r"(?i)\bdel\s+\/[sf]",
            r"(?i)\bformat\s+[a-z]:",
            r"\bdd\b.*\bof=",
            r"\bmkfs\b",
            r"\bfdisk\b",
            r"\bdiskpart\b",
            r">\s*/dev/",
            r"(?i)\bdrop\s+(database|table)\b",
            r"(?i)\btruncate\s+table\b",
            r"git\s+reset\s+--hard",
            r"git\s+clean\s+-[a-z]*f",
            r"git\s+push\b.*(--force|-f\b)",
            r"\bsudo\b",
            r"(^|\s)su\b",
            r"(?i)\brunas\b",
            r"chmod\s+-R",
            r"chown\s+-R",
            r"\bcurl\b[^|]*\|\s*(bash|sh)\b",
            r"\bwget\b[^|]*\|\s*(bash|sh)\b",
            r"npm\s+publish\b",
            r"cargo\s+publish\b",
            r"twine\s+upload\b",
            r"\b(shutdown|reboot|halt|poweroff)\b",
            r"kill\s+-9\b",
        ]
        .iter()
        .map(|p| Regex::new(p).unwrap())
        .collect()
    })
}

/// A first word that only reads, when the whole line is one command.
fn read_only_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^(ls|dir|cat|type|head|tail|less|more|echo|pwd|whoami|which|where|file|stat|tree|wc|grep|rg|find|fd|uname|node\s+--?v|npm\s+--?v|python\s+--?V|git\s+(status|diff|log|show|branch|stash\s+list|remote\s+-v))\b").unwrap()
    })
}

/// Editing one of these wants a second look.
fn sensitive_path_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)(\.env|secret|credential|\.pem$|\.key$|\b(auth|login|session|password|passwd|permission|acl)\b)").unwrap())
}

/// How much care an approval wants: destructive or privileged runs and edits
/// to sensitive files are high, plain reads and questions are low, and
/// everything else an agent runs (including compound lines) is mid.
pub fn risk_of_ask(kind: AskKind, detail: Option<&str>) -> AskRisk {
    let d = detail.unwrap_or("").trim();
    if risky_run_res().iter().any(|re| re.is_match(d)) {
        return AskRisk::High;
    }
    match kind {
        AskKind::Question => AskRisk::Low,
        AskKind::Edit => {
            if sensitive_path_re().is_match(d) {
                AskRisk::High
            } else {
                AskRisk::Mid
            }
        }
        AskKind::Run => {
            if !d.is_empty() && !d.contains(['&', ';']) && read_only_re().is_match(d) {
                AskRisk::Low
            } else {
                AskRisk::Mid
            }
        }
    }
}
