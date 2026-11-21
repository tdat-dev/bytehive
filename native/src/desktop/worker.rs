//! One thread owns UI Automation (COM objects stay on the thread that made
//! them): it runs each desktop step and keeps every agent's last outline, so
//! a ref like d12 means the element that outline listed.

use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use serde_json::Value;
use windows::Win32::Foundation::POINT;
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx};
use windows::Win32::UI::Accessibility::{CUIAutomation, IUIAutomation};

use super::tree::{self, Node};
use super::{act, keys, shot, words};

struct Job {
    agent: String,
    tool: String,
    args: Value,
    desk: Value,
    reply: Sender<Result<Value, String>>,
}

/// The last outline an agent read, and the window it came from.
struct Page {
    hwnd: isize,
    nodes: Vec<Node>,
}

pub fn ask(agent: &str, tool: &str, args: &Value, desk: &Value) -> Result<Value, String> {
    static TX: OnceLock<Mutex<Sender<Job>>> = OnceLock::new();
    let tx = TX.get_or_init(|| {
        let (tx, rx) = mpsc::channel();
        std::thread::Builder::new().name("bytehive-desktop".into()).spawn(move || serve(rx)).expect("desktop thread");
        Mutex::new(tx)
    });
    let (reply, answer) = mpsc::channel();
    let job = Job { agent: agent.into(), tool: tool.into(), args: args.clone(), desk: desk.clone(), reply };
    tx.lock().unwrap_or_else(|p| p.into_inner()).send(job).map_err(|_| "ByteHive's desktop helper stopped.".to_string())?;
    answer.recv_timeout(Duration::from_secs(80)).map_err(|_| "The step on this page took too long.".to_string())?
}

fn serve(rx: Receiver<Job>) {
    let uia: Option<IUIAutomation> = unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED).ok();
        CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER).ok()
    };
    let mut pages: HashMap<String, Page> = HashMap::new();
    for job in rx {
        let r = match &uia {
            None => Err("Windows UI Automation is not available here.".to_string()),
            Some(uia) => std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| handle(uia, &mut pages, &job))).unwrap_or_else(|_| Err("The step on this page failed inside ByteHive.".into())),
        };
        let _ = job.reply.send(r);
    }
}

/// The outline, once Chrome has built it: Chrome makes its accessibility tree
/// when someone first asks, so an empty first read is tried again.
fn outline(uia: &IUIAutomation, hwnd: isize, all: bool) -> Result<Vec<Node>, String> {
    let mut roots = tree::dialogs(hwnd);
    roots.push(hwnd);
    let mut nodes = tree::read(uia, &roots, all)?;
    for _ in 0..6 {
        if nodes.len() >= 3 {
            break;
        }
        std::thread::sleep(Duration::from_millis(400));
        nodes = tree::read(uia, &roots, all)?;
    }
    Ok(nodes)
}

fn lines(nodes: &[Node], origin: (i32, i32), keep: impl Fn(&Node) -> bool, max: usize) -> Vec<String> {
    nodes.iter().enumerate().filter(|(_, n)| keep(n)).take(max).map(|(i, n)| {
        let (x, y) = n.center();
        words::line(i + 1, n.kind, &n.name, x - origin.0, y - origin.1, &n.extra)
    }).collect()
}

fn handle(uia: &IUIAutomation, pages: &mut HashMap<String, Page>, job: &Job) -> Result<Value, String> {
    let hwnd = tree::find_window(&job.desk)?;
    let r = shot::rect(hwnd);
    let origin = (r.left, r.top);
    let url = job.desk["url"].as_str().unwrap_or("").to_string();
    let a = &job.args;
    let head = format!("{}\n{url}\nwindow {}x{}\n({})", job.desk["title"].as_str().unwrap_or(""), r.right - r.left, r.bottom - r.top, words::HEAD_NOTE);
    match job.tool.as_str() {
        "read_page" => {
            let nodes = outline(uia, hwnd, a["filter"] == "all")?;
            let max = a["max"].as_u64().unwrap_or(400) as usize;
            let body = lines(&nodes, origin, |_| true, max);
            pages.insert(job.agent.clone(), Page { hwnd, nodes });
            Ok(words::text(format!("{head}\n\n{}", body.join("\n"))))
        }
        "find" => {
            let q: Vec<String> = a["query"].as_str().unwrap_or("").to_lowercase().split_whitespace().map(String::from).collect();
            let nodes = outline(uia, hwnd, true)?;
            let hits = lines(&nodes, origin, |n| { let name = n.name.to_lowercase(); q.iter().all(|w| name.contains(w)) }, 30);
            pages.insert(job.agent.clone(), Page { hwnd, nodes });
            Ok(words::text(if hits.is_empty() { format!("Nothing on this page matches \"{}\".", a["query"].as_str().unwrap_or("")) } else { hits.join("\n") }))
        }
        "page_text" => {
            let nodes = outline(uia, hwnd, true)?;
            let max = a["max"].as_u64().unwrap_or(40000) as usize;
            let mut text: String = nodes.iter().filter(|n| !n.name.is_empty()).map(|n| n.name.as_str()).collect::<Vec<_>>().join("\n");
            if text.len() > max {
                text = text.chars().take(max).collect();
            }
            Ok(words::text(format!("{head}\n\n{text}")))
        }
        "form_input" => {
            let node = lookup(pages, &job.agent, hwnd, a["ref"].as_str().unwrap_or(""))?;
            if let Some(h) = guard(a, &format!("Set {} \"{}\"", node.kind, node.name), &node.name, &url) {
                return Ok(h);
            }
            act::set_value(node, &a["value"]).map(words::text)
        }
        "computer" => computer(uia, pages, job, hwnd, origin, &url),
        "upload" => Err(words::NO_UPLOAD.into()),
        "javascript" | "console" | "network" | "dialog" => Err(words::NO_SCRIPT.into()),
        other => Err(format!("{other} is not available on this page. {}", words::NO_SCRIPT)),
    }
}

fn lookup<'a>(pages: &'a HashMap<String, Page>, agent: &str, hwnd: isize, r: &str) -> Result<&'a Node, String> {
    let i = words::parse_ref(r)?;
    let page = pages.get(agent).filter(|p| p.hwnd == hwnd).ok_or("Read this page first: read_page or find gives the refs.")?;
    page.nodes.get(i).ok_or_else(|| format!("No {r} on this page: read_page again."))
}

/// A risky step on a guarded page waits for the user, unless they already said yes.
fn guard(a: &Value, what: &str, name: &str, url: &str) -> Option<Value> {
    (a["_ask"] == true && a["confirmed"] != true && words::risky(name)).then(|| words::hold(what, url))
}

/// The name of whatever is at a screen point.
fn name_at(uia: &IUIAutomation, x: i32, y: i32) -> String {
    unsafe { uia.ElementFromPoint(POINT { x, y }).and_then(|e| e.CurrentName()).map(|n| n.to_string()).unwrap_or_default() }
}

fn computer(uia: &IUIAutomation, pages: &HashMap<String, Page>, job: &Job, hwnd: isize, origin: (i32, i32), url: &str) -> Result<Value, String> {
    let a = &job.args;
    let act = a["action"].as_str().unwrap_or("");
    match act {
        "screenshot" => return shot::screenshot(hwnd),
        "wait" => {
            std::thread::sleep(Duration::from_secs_f64(a["duration"].as_f64().unwrap_or(1.0).clamp(0.0, 30.0)));
            return Ok(words::text("Waited."));
        }
        "type" => {
            keys::ensure_front(hwnd)?;
            keys::type_text(a["text"].as_str().unwrap_or(""));
            return Ok(words::text("Typed."));
        }
        "key" => {
            let spec = a["text"].as_str().unwrap_or("");
            let combos = keys::parse(spec)?;
            let confirms = combos.iter().any(|c| c.contains(&0x0D) || c.contains(&0x20));
            if confirms {
                let focused = unsafe { uia.GetFocusedElement().and_then(|e| e.CurrentName()).map(|n| n.to_string()).unwrap_or_default() };
                if let Some(h) = guard(a, &format!("Press {spec} on \"{focused}\""), &focused, url) {
                    return Ok(h);
                }
            }
            keys::ensure_front(hwnd)?;
            keys::press(&combos);
            return Ok(words::text(format!("Pressed {spec}.")));
        }
        _ => {}
    }
    // Everything else has a target: a ref, or a point of the screenshot.
    let node = match a["ref"].as_str() {
        Some(r) => Some(lookup(pages, &job.agent, hwnd, r)?),
        None => None,
    };
    let (x, y) = match (node, a["coordinate"].as_array()) {
        (_, Some(c)) if c.len() == 2 => (origin.0 + c[0].as_f64().unwrap_or(0.0) as i32, origin.1 + c[1].as_f64().unwrap_or(0.0) as i32),
        (Some(n), _) => n.center(),
        _ => return Err("Give a ref (from read_page) or a coordinate [x, y] (from a screenshot).".into()),
    };
    match act {
        "hover" => {
            keys::move_to(x, y);
            Ok(words::text("Hovering."))
        }
        "scroll" => {
            keys::ensure_front(hwnd)?;
            let n = a["scroll_amount"].as_i64().unwrap_or(3) as i32;
            let dir = a["scroll_direction"].as_str().unwrap_or("down");
            let notches = if matches!(dir, "up" | "left") { -n } else { n };
            keys::wheel(x, y, notches, matches!(dir, "left" | "right"));
            Ok(words::text(format!("Scrolled {dir}.")))
        }
        "left_click" | "right_click" | "double_click" | "triple_click" => {
            let name = node.map(|n| n.name.clone()).unwrap_or_else(|| name_at(uia, x, y));
            if act == "left_click" {
                if let Some(h) = guard(a, &format!("Click \"{name}\""), &name, url) {
                    return Ok(h);
                }
            }
            // A ref, clicked once: pressed the way assistive tools do.
            if let (Some(n), "left_click", None) = (node, act, a["coordinate"].as_array()) {
                match act::press(n) {
                    Ok(how) => {
                        std::thread::sleep(Duration::from_millis(400));
                        return Ok(words::text(format!("{how} {} \"{}\".", n.kind, n.name)));
                    }
                    Err(Some(e)) => return Err(e),
                    Err(None) => {}
                }
            }
            keys::ensure_front(hwnd)?;
            let clicks = match act { "double_click" => 2, "triple_click" => 3, _ => 1 };
            keys::click(x, y, act == "right_click", clicks);
            std::thread::sleep(Duration::from_millis(400));
            Ok(words::text(format!("{act} on \"{name}\" at {},{}.", x - origin.0, y - origin.1)))
        }
        other => Err(format!("Unknown action \"{other}\".")),
    }
}
