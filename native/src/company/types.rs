//! The Company's files, field for field as the Tauri app wrote them (types.ts),
//! so the MCP server and a company made before carry over. Unknown fields on the
//! company, a floor, a person, a look, the limits, a plan and a Job are kept, and
//! so is a rank or a status this build does not know, so a newer build's additions
//! survive an older build's save.

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;

pub const FOUNDER_ID: &str = "founder";

/// An enum written as a string. A value this build does not know is kept in
/// `Other` and written back as it was.
macro_rules! open_enum {
    ($(#[$m:meta])* $name:ident { $($v:ident = $s:literal),* $(,)? }) => {
        $(#[$m])*
        #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(from = "String", into = "String")]
        pub enum $name {
            $($v,)*
            Other(String),
        }

        impl $name {
            pub fn as_str(&self) -> &str {
                match self {
                    $(Self::$v => $s,)*
                    Self::Other(s) => s,
                }
            }
        }

        impl From<String> for $name {
            fn from(s: String) -> Self {
                match s.as_str() {
                    $($s => Self::$v,)*
                    _ => Self::Other(s),
                }
            }
        }

        impl From<$name> for String {
            fn from(v: $name) -> String {
                v.as_str().to_string()
            }
        }
    };
}

open_enum!(Rank { Founder = "founder", Lead = "lead", Staff = "staff" });

/// A field the file leaves out reads as empty.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Look {
    pub shirt: String,
    pub hair: String,
    pub prop: String,
    /// The owner chose this look.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edited: Option<bool>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Floor {
    pub id: String,
    pub dir: String,
    pub name: String,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Person {
    pub id: String,
    pub name: String,
    pub role: String,
    pub rank: Rank,
    /// None for the Founder.
    pub floor: Option<String>,
    /// A Person id, None for the Founder.
    pub boss: Option<String>,
    pub cli: String,
    pub look: Look,
    /// What their pane runs on, so a closed pane comes back on it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worktree: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A limit a file leaves out takes its default, so a limit added later reads in.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Limits {
    pub people_per_job: u32,
    pub people_per_lead: u32,
    pub people_total: u32,
    pub tokens_per_job: u64,
    pub stall_minutes: u32,
    pub max_returns: u32,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

pub const DEFAULT_LIMITS: Limits = Limits {
    people_per_job: 5,
    people_per_lead: 5,
    people_total: 20,
    tokens_per_job: 5_000_000,
    stall_minutes: 10,
    max_returns: 3,
    extra: BTreeMap::new(),
};

impl Default for Limits {
    fn default() -> Self {
        DEFAULT_LIMITS
    }
}

/// `"limits": null` reads as the defaults, as `{ ...DEFAULT_LIMITS, ...null }` does.
fn limits_or_default<'de, D: Deserializer<'de>>(d: D) -> Result<Limits, D::Error> {
    Ok(Option::<Limits>::deserialize(d)?.unwrap_or_default())
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Company {
    /// Always 1: a file with another version is refused (store::parse_company).
    pub version: u32,
    pub floors: Vec<Floor>,
    pub people: Vec<Person>,
    #[serde(default, deserialize_with = "limits_or_default")]
    pub limits: Limits,
    #[serde(default = "first_job")]
    pub next_job: u64,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

fn first_job() -> u64 {
    1
}

impl Company {
    /// A company with only the Founder in it (org.ts emptyCompany).
    pub fn empty() -> Self {
        let founder = Person {
            id: FOUNDER_ID.into(),
            name: "Founder".into(),
            role: "founder".into(),
            rank: Rank::Founder,
            floor: None,
            boss: None,
            cli: "claude".into(),
            look: Look { shirt: "#f5b83d".into(), hair: "#2a2e3a".into(), prop: "none".into(), edited: None, extra: Map::new() },
            session: None,
            worktree: None,
            branch: None,
            extra: Map::new(),
        };
        Company { version: 1, floors: Vec::new(), people: vec![founder], limits: DEFAULT_LIMITS, next_job: 1, extra: Map::new() }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoleDef {
    pub id: String,
    pub title: String,
    pub department: String,
    pub rank: Rank,
    pub cli: String,
    pub deny_tools: Vec<String>,
    pub look: Look,
    pub prompt: String,
    /// Folder names under the role's skills/.
    pub skills: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hire {
    pub role: String,
    pub floor: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlanCard {
    pub floor: String,
    pub title: String,
    pub desc: String,
    pub checklist: Vec<String>,
    /// A Person name, an existing person or left for the Lead.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assignee: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A role the Founder drafts in a plan. The rest of a RoleDef is the app's to set.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewRole {
    pub id: String,
    pub title: String,
    pub department: String,
    pub prompt: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobPlan {
    pub summary: String,
    pub cards: Vec<PlanCard>,
    pub hires: Vec<Hire>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_cap: Option<u64>,
    /// Saved only when the owner approves the plan.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new_roles: Option<Vec<NewRole>>,
    /// Person ids already on a floor who take a hire's work: set by the app, never by the Founder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reuse: Option<Vec<String>>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

open_enum!(JobStatus {
    Planning = "planning",
    AwaitingPlan = "awaiting-plan",
    Running = "running",
    AwaitingMerge = "awaiting-merge",
    Paused = "paused",
    Stopped = "stopped",
    Done = "done",
    Rejected = "rejected",
});

/// Only id, status and people are required, as loadJobs checks; the rest
/// takes a default when a file leaves it out.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Job {
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub ask: String,
    pub status: JobStatus,
    #[serde(default)]
    pub plan: Option<JobPlan>,
    #[serde(default)]
    pub token_cap: u64,
    #[serde(default)]
    pub tokens_used: u64,
    /// Person id to the fresh-token figure last seen for them, to count what they add.
    #[serde(default)]
    pub token_base: BTreeMap<String, u64>,
    /// Person ids working this Job.
    pub people: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(default)]
    pub created_at: u64,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// A line of requests.jsonl, written by the MCP server.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum CompanyRequest {
    Message { ts: u64, from: String, to: Option<String>, message: String },
    Plan { ts: u64, from: String, job: String, plan: JobPlan },
    JobDone { ts: u64, from: String, job: String, summary: String },
}
