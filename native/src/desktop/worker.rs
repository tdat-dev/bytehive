//! One thread owns UI Automation (COM objects stay on the thread that made
//! them): it runs each desktop step and keeps every agent's last outline, so
//! a ref like d12 means the element that outline listed.

use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use serde_json::Value;
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx};
use windows::Win32::UI::Accessibility::{CUIAutomation, IUIAutomation};

use super::tree::{self, Node};
use super::{act, shot, words};

pub(super) struct Job {
    pub agent: String,
    tool: String,
    pub args: Value,
    desk: Value,
    reply: Sender<Result<Value, String>>,
}

/// The last outline an agent read, the window it came from, and the field it
/// last picked (where `type` goes without keys).
pub(super) struct Page {
    pub hwnd: isize,
    pub nodes: Vec<Node>,
    pub field: Option<usize>,
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
    super::idle::watch();
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
            pages.insert(job.agent.clone(), Page { hwnd, nodes, field: None });
            Ok(words::text(format!("{head}\n\n{}", body.join("\n"))))
        }
        "find" => {
            let q: Vec<String> = a["query"].as_str().unwrap_or("").to_lowercase().split_whitespace().map(String::from).collect();
            let nodes = outline(uia, hwnd, true)?;
            let hits = lines(&nodes, origin, |n| { let name = n.name.to_lowercase(); q.iter().all(|w| name.contains(w)) }, 30);
            pages.insert(job.agent.clone(), Page { hwnd, nodes, field: None });
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
            let i = index(pages, &job.agent, hwnd, a["ref"].as_str().unwrap_or(""))?;
            let page = pages.get_mut(&job.agent).ok_or("Read this page first.")?;
            let node = &page.nodes[i];
            if let Some(h) = guard(a, &format!("Set {} \"{}\"", node.kind, node.name), &node.name, &url) {
                return Ok(h);
            }
            // Text makes Chrome come to the front: done while the person is idle, their window given back.
            let done = if a["value"].is_boolean() { act::set_value(node, &a["value"])? } else { super::keys::settle(hwnd, || act::set_value(node, &a["value"]))? };
            if act::writable(&node.el) {
                page.field = Some(i);
            }
            Ok(words::text(done))
        }
        "computer" => super::computer::run(uia, pages, job, hwnd, origin, &url),
        "upload" => Err(words::NO_UPLOAD.into()),
        "javascript" | "console" | "network" | "dialog" => Err(words::NO_SCRIPT.into()),
        other => Err(format!("{other} is not available on this page. {}", words::NO_SCRIPT)),
    }
}

/// Where a ref sits in the agent's last outline of this window.
pub(super) fn index(pages: &HashMap<String, Page>, agent: &str, hwnd: isize, r: &str) -> Result<usize, String> {
    let i = words::parse_ref(r)?;
    let page = pages.get(agent).filter(|p| p.hwnd == hwnd).ok_or("Read this page first: read_page or find gives the refs.")?;
    if i < page.nodes.len() { Ok(i) } else { Err(format!("No {r} on this page: read_page again.")) }
}

/// A risky step on a guarded page waits for the user, unless they already said yes.
pub(super) fn guard(a: &Value, what: &str, name: &str, url: &str) -> Option<Value> {
    (a["_ask"] == true && a["confirmed"] != true && words::risky(name)).then(|| words::hold(what, url))
}
