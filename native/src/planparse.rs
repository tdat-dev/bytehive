//! An agent's plan file (.bytehive/plan.json) read into tasks (planparse.ts).
//! Takes a JSON array of tasks, a {tasks:[...]} wrapper, or a plain markdown
//! checklist or bullet list: agents don't always write well-shaped JSON.

use std::sync::OnceLock;

use regex::Regex;
use serde_json::Value;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlanTask {
    pub title: String,
    pub desc: Option<String>,
    pub label: Option<String>,
    /// Small steps: they become the card's checklist.
    pub subtasks: Option<Vec<String>>,
}

const LABEL_KEYS: [&str; 6] = ["green", "yellow", "orange", "red", "purple", "blue"];

fn normalize_task(raw: &Value) -> Option<PlanTask> {
    if let Value::String(s) = raw {
        let title = s.trim();
        return (!title.is_empty()).then(|| PlanTask { title: title.to_string(), ..PlanTask::default() });
    }
    let o = raw.as_object()?;
    let text = |key: &str| o.get(key).and_then(Value::as_str);
    let title = text("title").or_else(|| text("task")).unwrap_or("").trim();
    if title.is_empty() {
        return None;
    }
    let mut t = PlanTask { title: title.to_string(), ..PlanTask::default() };
    let filled = |key: &str| text(key).map(str::trim).filter(|s| !s.is_empty()).map(str::to_string);
    t.desc = filled("desc").or_else(|| filled("detail"));
    t.label = text("label").filter(|l| LABEL_KEYS.contains(l)).map(str::to_string);
    // A big task carries its small steps: "subtasks" (or "checklist" or
    // "steps"), strings that become the card's checklist.
    let steps = ["subtasks", "checklist", "steps"].iter().find_map(|k| o.get(*k).filter(|v| !v.is_null()));
    if let Some(Value::Array(steps)) = steps {
        let steps: Vec<String> = steps.iter().filter_map(|s| s.as_str()).map(str::trim).filter(|s| !s.is_empty()).map(str::to_string).collect();
        if !steps.is_empty() {
            t.subtasks = Some(steps);
        }
    }
    Some(t)
}

/// A plan file's text as tasks, JSON or a markdown list.
pub fn parse_plan(text: &str) -> Vec<PlanTask> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    // 1) A JSON array, or { tasks: [...] }.
    if let Ok(j) = serde_json::from_str::<Value>(trimmed) {
        let arr = match &j {
            Value::Array(a) => Some(a),
            Value::Object(o) => o.get("tasks").and_then(Value::as_array),
            _ => None,
        };
        if let Some(arr) = arr {
            return arr.iter().filter_map(normalize_task).collect();
        }
    }
    // 2) A markdown checklist or bullet list: "- [ ] task", "- task", "* task", "1. task".
    static ITEM: OnceLock<Regex> = OnceLock::new();
    let item = ITEM.get_or_init(|| Regex::new(r"^\s*(?:[-*]|[0-9]+[.)])\s+(?:\[[ xX]\]\s+)?(.+?)\s*$").expect("valid pattern"));
    trimmed
        .split('\n')
        .map(|l| l.strip_suffix('\r').unwrap_or(l))
        .filter_map(|l| item.captures(l).and_then(|m| m.get(1)).map(|t| PlanTask { title: t.as_str().to_string(), ..PlanTask::default() }))
        .collect()
}

#[cfg(test)]
mod tests {
    //! Ported from src/planparse.test.ts.

    use super::*;

    fn task(title: &str) -> PlanTask {
        PlanTask { title: title.to_string(), ..PlanTask::default() }
    }

    fn steps(title: &str, s: &[&str]) -> PlanTask {
        PlanTask { subtasks: Some(s.iter().map(|x| x.to_string()).collect()), ..task(title) }
    }

    #[test]
    fn parses_a_json_array_of_task_objects() {
        let got = parse_plan(r#"[{"title":"Add auth","desc":"jwt","label":"blue"},{"title":"Tests"}]"#);
        assert_eq!(got, vec![PlanTask { desc: Some("jwt".into()), label: Some("blue".into()), ..task("Add auth") }, task("Tests")]);
    }

    #[test]
    fn accepts_a_tasks_wrapper_and_string_items() {
        assert_eq!(parse_plan(r#"{"tasks":["one","two"]}"#), vec![task("one"), task("two")]);
    }

    #[test]
    fn drops_invalid_labels_but_keeps_the_task() {
        assert_eq!(parse_plan(r#"[{"title":"x","label":"chartreuse"}]"#), vec![task("x")]);
    }

    #[test]
    fn falls_back_to_a_markdown_checklist() {
        let md = "# Plan\n- [ ] First task\n- [x] Second task\n* Third\n1. Fourth";
        let titles: Vec<String> = parse_plan(md).into_iter().map(|t| t.title).collect();
        assert_eq!(titles, vec!["First task", "Second task", "Third", "Fourth"]);
    }

    #[test]
    fn keeps_subtasks_or_checklist_or_steps_as_the_cards_checklist() {
        let got = parse_plan(r#"[{"title":"Build login","subtasks":["form UI","API call","  ", 7,"error states"]},{"title":"Ship","steps":["bump version"]}]"#);
        assert_eq!(got, vec![steps("Build login", &["form UI", "API call", "error states"]), steps("Ship", &["bump version"])]);
    }

    #[test]
    fn returns_nothing_for_empty_or_junk() {
        assert!(parse_plan("").is_empty());
        assert!(parse_plan("just a sentence with no list").is_empty());
    }

    #[test]
    fn task_and_detail_stand_in_and_crlf_lines_parse() {
        assert_eq!(parse_plan(r#"[{"task":"t","detail":" d "}]"#), vec![PlanTask { desc: Some("d".into()), ..task("t") }]);
        assert_eq!(parse_plan("- a\r\n2) b\r\n"), vec![task("a"), task("b")]);
    }
}
