//! An OpenCode session read whole, as plain events: what the person said,
//! what the agent answered, the tools it used and how they ended. OpenCode
//! keeps two stores in one database: the newer `session_v2` / `session_message`
//! (a row per message, its parts inside) and the older `session` / `message` /
//! `part`. A session lives in one of them; both are read here, read only.

use std::path::Path;
use std::time::Duration;

use rusqlite::{params, Connection, OpenFlags};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    User(String),
    Text(String),
    Tool { name: String, input: Value, failed: bool, done: bool },
}

fn s<'a>(v: &'a Value, k: &str) -> &'a str {
    v.get(k).and_then(Value::as_str).unwrap_or("")
}

/// A tool part's event from its name and state (the same `state` in both stores).
fn tool(name: &str, state: &Value) -> Event {
    let status = s(state, "status");
    let exit = state.pointer("/metadata/exit").and_then(Value::as_i64);
    Event::Tool {
        name: name.to_string(),
        input: state.get("input").cloned().unwrap_or(Value::Null),
        failed: status == "error" || exit.is_some_and(|c| c != 0),
        done: matches!(status, "completed" | "error"),
    }
}

/// The newer store: one row per message, ordered by `seq`.
fn v2(conn: &Connection, session: &str) -> rusqlite::Result<Vec<Event>> {
    let mut stmt = conn.prepare("SELECT type, data FROM session_message WHERE session_id = ?1 ORDER BY seq")?;
    let rows = stmt.query_map(params![session], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
    let mut out = Vec::new();
    for row in rows {
        let (kind, data) = row?;
        let Ok(d) = serde_json::from_str::<Value>(&data) else { continue };
        match kind.as_str() {
            "user" => {
                let t = s(&d, "text").trim();
                if !t.is_empty() {
                    out.push(Event::User(t.to_string()));
                }
            }
            "assistant" => {
                for c in d.get("content").and_then(Value::as_array).into_iter().flatten() {
                    match s(c, "type") {
                        "text" if !s(c, "text").trim().is_empty() => out.push(Event::Text(s(c, "text").trim().to_string())),
                        "tool" => out.push(tool(s(c, "name"), c.get("state").unwrap_or(&Value::Null))),
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    Ok(out)
}

/// The older store: parts joined to their message for its role, in the order written.
fn v1(conn: &Connection, session: &str) -> rusqlite::Result<Vec<Event>> {
    let mut stmt = conn.prepare(
        "SELECT m.data, p.data FROM part p JOIN message m ON m.id = p.message_id
         WHERE p.session_id = ?1 ORDER BY m.time_created, m.id, p.time_created, p.id",
    )?;
    let rows = stmt.query_map(params![session], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
    let mut out = Vec::new();
    for row in rows {
        let (m, p) = row?;
        let (Ok(m), Ok(p)) = (serde_json::from_str::<Value>(&m), serde_json::from_str::<Value>(&p)) else { continue };
        match s(&p, "type") {
            "text" if p.get("synthetic") != Some(&Value::Bool(true)) => {
                let t = s(&p, "text").trim();
                if t.is_empty() {
                    continue;
                }
                out.push(if s(&m, "role") == "user" { Event::User(t.to_string()) } else { Event::Text(t.to_string()) });
            }
            "tool" => out.push(tool(s(&p, "tool"), p.get("state").unwrap_or(&Value::Null))),
            _ => {}
        }
    }
    Ok(out)
}

/// Every event of `session`, from whichever store holds it; empty when neither does.
pub fn session_events(db: &Path, session: &str) -> Vec<Event> {
    let Ok(conn) = Connection::open_with_flags(db, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX) else { return Vec::new() };
    let _ = conn.busy_timeout(Duration::from_millis(1500));
    let in_v2 = conn.query_row("SELECT 1 FROM session_v2 WHERE id = ?1", params![session], |_| Ok(())).is_ok();
    let read = if in_v2 { v2(&conn, session) } else { v1(&conn, session) };
    read.unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db(name: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("oc-events-{name}-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let c = Connection::open(&path).unwrap();
        c.execute_batch(
            "CREATE TABLE session_v2 (id TEXT);
             CREATE TABLE session_message (id TEXT, session_id TEXT, type TEXT, seq INTEGER, data TEXT);
             CREATE TABLE message (id TEXT, session_id TEXT, time_created INTEGER, data TEXT);
             CREATE TABLE part (id TEXT, message_id TEXT, session_id TEXT, time_created INTEGER, data TEXT);",
        )
        .unwrap();
        path
    }

    #[test]
    fn reads_a_new_store_session() {
        let path = db("v2");
        let c = Connection::open(&path).unwrap();
        c.execute("INSERT INTO session_v2 VALUES ('ses_2')", []).unwrap();
        c.execute("INSERT INTO session_message VALUES ('a','ses_2','user',1,'{\"text\":\"fix it\"}')", []).unwrap();
        let asst = r#"{"content":[{"type":"reasoning","text":"hm"},{"type":"tool","name":"shell","state":{"status":"completed","input":{"command":"npm test"},"metadata":{"exit":1}}},{"type":"text","text":"Tests fail."}]}"#;
        c.execute("INSERT INTO session_message VALUES ('b','ses_2','assistant',2,?1)", params![asst]).unwrap();
        drop(c);
        let ev = session_events(&path, "ses_2");
        assert_eq!(ev[0], Event::User("fix it".into()));
        assert!(matches!(&ev[1], Event::Tool { name, failed: true, done: true, .. } if name == "shell"));
        assert_eq!(ev[2], Event::Text("Tests fail.".into()));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn reads_an_old_store_session_and_skips_synthetic_text() {
        let path = db("v1");
        let c = Connection::open(&path).unwrap();
        c.execute("INSERT INTO message VALUES ('m1','ses_1',1,'{\"role\":\"user\"}')", []).unwrap();
        c.execute("INSERT INTO message VALUES ('m2','ses_1',2,'{\"role\":\"assistant\"}')", []).unwrap();
        c.execute("INSERT INTO part VALUES ('p1','m1','ses_1',1,'{\"type\":\"text\",\"text\":\"hello\"}')", []).unwrap();
        c.execute("INSERT INTO part VALUES ('p0','m1','ses_1',1,'{\"type\":\"text\",\"text\":\"ctx\",\"synthetic\":true}')", []).unwrap();
        c.execute("INSERT INTO part VALUES ('p2','m2','ses_1',3,'{\"type\":\"tool\",\"tool\":\"edit\",\"state\":{\"status\":\"error\",\"input\":{\"filePath\":\"a.rs\"}}}')", []).unwrap();
        drop(c);
        let ev = session_events(&path, "ses_1");
        assert_eq!(ev.len(), 2);
        assert_eq!(ev[0], Event::User("hello".into()));
        assert!(matches!(&ev[1], Event::Tool { name, failed: true, .. } if name == "edit"));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_missing_database_or_session_is_empty() {
        assert!(session_events(Path::new("Z:/no/such.db"), "ses_x").is_empty());
        let path = db("none");
        assert!(session_events(&path, "ses_missing").is_empty());
        let _ = std::fs::remove_file(&path);
    }
}
