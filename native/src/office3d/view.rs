//! The office in the Company page: a canvas that draws the scene at its own
//! size and shows the frame as an image (GPUI on Windows cannot host a GPU
//! surface). It draws only when something asks: a new model, a level, the
//! pointer, or the scene's own clock (people at work, a plane in the air).

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use gpui_kit::*;

use super::layout::Level;
use super::quality::Quality;
use super::scene::Scene;
use super::stage::{Anchor, What};
use super::world_in::OfficeIn;

struct Live {
    scene: Result<Scene, String>,
    shown: Option<Arc<RenderImage>>,
    /// The frame before, dropped a frame late so GPUI's atlas never empties.
    before: Option<Arc<RenderImage>>,
    bounds: Option<Bounds<Pixels>>,
    started: Instant,
    /// A timer is out to ask for the next frame, and one came due.
    waiting: bool,
    due: bool,
    /// Something changed since the last frame.
    dirty: bool,
}

thread_local! {
    static LIVE: RefCell<Option<Live>> = const { RefCell::new(None) };
}

/// What the page does with a click on the office.
pub type OnPick = Rc<dyn Fn(What, &mut Window, &mut App)>;

fn wall_ms() -> f64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0.0, |d| d.as_secs_f64() * 1000.0)
}

fn with<R>(f: impl FnOnce(&mut Live) -> R) -> R {
    LIVE.with(|cell| {
        let mut cell = cell.borrow_mut();
        let live = cell.get_or_insert_with(|| Live { scene: Scene::new(Quality::Auto, false), shown: None, before: None, bounds: None, started: Instant::now(), waiting: false, due: false, dirty: true });
        f(live)
    })
}

/// Why the office cannot be drawn, if it cannot.
pub fn broken() -> Option<String> {
    with(|l| l.scene.as_ref().err().cloned())
}

/// The model and the level to show. Cheap when nothing changed.
pub fn set(model: &OfficeIn, level: &Level, reduced: bool) {
    with(|l| {
        let Ok(s) = l.scene.as_mut() else { return };
        s.reduced = reduced;
        let mut changed = s.set_model(model.clone(), wall_ms());
        if s.level() != level {
            s.set_level(level.clone(), false);
            changed = true;
        }
        l.dirty |= changed;
    });
}

/// What the office has done, for a line over it while it is ported.
pub fn line() -> String {
    with(|l| match &l.scene {
        Err(e) => format!("3D off: {e}"),
        Ok(s) => format!("{} frames, {} shown, on {}", s.frames, if l.shown.is_some() { "one" } else { "none" }, s.adapter()),
    })
}

/// Where the words go over the picture, in the canvas's own pixels.
pub fn anchors() -> Vec<Anchor> {
    with(|l| l.scene.as_ref().map(Scene::anchors).unwrap_or_default())
}

/// Leaves this much room on the left at the Building, for the job box.
pub fn set_clear(px: f64) {
    with(|l| {
        if let Ok(s) = l.scene.as_mut() {
            s.set_clear(px);
        }
    });
}

fn local(l: &Live, p: Point<Pixels>) -> Option<(f64, f64, f64, f64)> {
    let b = l.bounds?;
    let (x, y) = (f32::from(p.x - b.origin.x) as f64, f32::from(p.y - b.origin.y) as f64);
    Some((x, y, f32::from(b.size.width) as f64, f32::from(b.size.height) as f64))
}

/// The canvas, with the pointer and a click handed to the scene.
pub fn view(on_pick: OnPick) -> impl IntoElement {
    let moved = |e: &MouseMoveEvent, window: &mut Window, _: &mut App| {
        let again = with(|l| {
            let Some((x, y, w, h)) = local(l, e.position) else { return false };
            let Ok(s) = l.scene.as_mut() else { return false };
            let who = s.pick(x, y).and_then(|w| w.person);
            let a = s.hover(who.as_deref());
            let b = s.set_parallax(x / w * 2.0 - 1.0, y / h * 2.0 - 1.0);
            l.dirty |= a || b;
            a || b
        });
        if again {
            window.refresh();
        }
    };
    let clicked = move |e: &MouseDownEvent, window: &mut Window, cx: &mut App| {
        let hit = with(|l| {
            let (x, y, ..) = local(l, e.position)?;
            l.scene.as_ref().ok()?.pick(x, y)
        });
        if let Some(w) = hit {
            on_pick(w, window, cx);
        }
    };
    div()
        .id("office3d")
        .size_full()
        .on_mouse_move(moved)
        .on_mouse_down(MouseButton::Left, clicked)
        .on_hover(|on, window, _| {
            if !on {
                with(|l| {
                    if let Ok(s) = l.scene.as_mut() {
                        l.dirty |= s.hover(None);
                    }
                });
                window.refresh();
            }
        })
        .child(canvas(|_, _, _| {}, paint).size_full())
}

fn paint(bounds: Bounds<Pixels>, _: (), window: &mut Window, cx: &mut App) {
    let scale = window.scale_factor() as f64;
    let (w, h) = (f32::from(bounds.size.width) as f64, f32::from(bounds.size.height) as f64);
    let (image, old, next) = with(|l| {
        l.bounds = Some(bounds);
        let Ok(s) = l.scene.as_mut() else { return (None, None, None) };
        s.resize(w, h, scale);
        let now = wall_ms();
        let t = l.started.elapsed().as_secs_f64() * 1000.0;
        // A frame back from the card first: it is the one to show.
        let (mut back, _) = s.collect();
        let tween = s.next_frame(now).is_some_and(|ms| ms <= 0.0);
        if l.dirty || l.due || tween {
            l.dirty = false;
            l.due = false;
            if let Ok(Some(px)) = s.frame(now, t) {
                back = Some(px);
            }
        }
        let mut old = None;
        if let Some((fw, fh, bytes)) = back
            && let Some(buffer) = image::ImageBuffer::from_raw(fw, fh, bytes)
        {
            let fresh = Arc::new(RenderImage::new(vec![image::Frame::new(buffer)]));
            old = std::mem::replace(&mut l.before, l.shown.replace(fresh));
        }
        // A frame still on the card comes back on the next paint.
        let (_, busy) = s.collect();
        let next = if busy || tween { Some(0.0) } else { s.next_frame(now) };
        (l.shown.clone(), old, next)
    });
    if let Some(old) = old {
        let _ = window.drop_image(old);
    }
    if let Some(image) = image {
        let _ = window.paint_image(bounds, bounds, Corners::default(), image, 0, false);
    }
    match next {
        Some(ms) if ms <= 0.0 => window.request_animation_frame(),
        Some(ms) => {
            let start = with(|l| !std::mem::replace(&mut l.waiting, true));
            if start {
                let handle = window.window_handle();
                cx.spawn(async move |cx| {
                    cx.background_executor().timer(Duration::from_millis(ms as u64)).await;
                    with(|l| {
                        l.waiting = false;
                        l.due = true;
                    });
                    let _ = handle.update(cx, |_, window, _| window.refresh());
                })
                .detach();
            }
        }
        None => {}
    }
}

/// BYTEHIVE_OFFICE3D=demo: the demo company in place of the floors, to look at.
pub fn demo_wanted() -> bool {
    std::env::var("BYTEHIVE_OFFICE3D").is_ok_and(|v| v == "demo")
}

/// The demo company, a click on someone or a floor going to that level and back up from there.
pub fn demo() -> impl IntoElement {
    thread_local! {
        static LEVEL: RefCell<Level> = const { RefCell::new(Level::Building) };
        static MODEL: RefCell<Option<OfficeIn>> = const { RefCell::new(None) };
    }
    let model = MODEL.with(|m| m.borrow_mut().get_or_insert_with(|| super::demo::demo_office(wall_ms())).clone());
    let level = LEVEL.with(|l| l.borrow().clone());
    set(&model, &level, false);
    view(Rc::new(|w: What, window: &mut Window, _: &mut App| {
        LEVEL.with(|l| {
            let mut l = l.borrow_mut();
            *l = match (&*l, w.person, w.floor) {
                (Level::Building, _, Some(f)) => Level::Floor(f),
                (Level::Floor(at), Some(p), Some(f)) if *at == f => Level::Desk { person: p, floor: Some(f) },
                (Level::Building | Level::Floor(_), Some(p), None) => Level::Desk { person: p, floor: None },
                (Level::Floor(_), _, Some(f)) => Level::Floor(f),
                _ => Level::Building,
            };
        });
        window.refresh();
    }))
}

