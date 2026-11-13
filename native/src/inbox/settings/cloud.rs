//! Settings → Save sessions to Remote: the switch, signing in with GitHub, the
//! passphrase and a status line (docs/superpowers/specs/2026-11-13-remote-session-store.md).
//! The slow parts (the sign-in wait, PBKDF2, the relay) run off the UI thread.

use super::rows::{button, row, text, toggle};
use super::sections::section;
use super::*;
use crate::cloud::{crypto, relay, secret, sync};

const MIN_PASS: usize = 10;

/// The two passphrase boxes, while open.
pub(in crate::inbox) struct PassForm {
    first: Entity<InputState>,
    again: Entity<InputState>,
    error: Option<String>,
}

#[derive(Default)]
pub(in crate::inbox) struct CloudUi {
    saved: secret::Saved,
    loaded: bool,
    form: Option<PassForm>,
    /// A sign-in or a passphrase is being worked on.
    busy: bool,
    note: Option<String>,
}

impl CloudUi {
    /// At start: what was saved, and the saving thread (idle while Remote is off).
    pub(in crate::inbox) fn load() -> Self {
        sync::start();
        Self { saved: secret::load(), loaded: true, ..Self::default() }
    }
}

/// The folder's last part: what a project is called in a list.
fn folder_name(dir: &str) -> String {
    dir.trim_end_matches(['\\', '/']).rsplit(['\\', '/']).next().unwrap_or(dir).to_string()
}

/// What is wrong with a passphrase typed twice, or None when it will do.
fn pass_problem(first: &str, again: &str) -> Option<String> {
    if first.chars().count() < MIN_PASS {
        return Some(format!("Use at least {MIN_PASS} characters."));
    }
    (first != again).then(|| "The two don't match.".into())
}

/// The keys for `pass` against the account's vault, making the vault when there is none.
/// `replace` sets a new passphrase over an old one: everything stored goes.
fn vault_keys(token: &str, pass: &str, replace: bool) -> Result<crypto::Keys, String> {
    if !replace && let Some(v) = relay::vault(token).map_err(|e| e.words())? {
        let salt = crypto::unb64(&v.salt).ok_or("The account's vault is damaged.")?;
        let keys = crypto::derive(pass, &salt, v.rounds);
        return if crypto::checks(&keys, &v.check) {
            Ok(keys)
        } else {
            Err("This account already has another passphrase. Type that one, or choose Change to start over.".into())
        };
    }
    let salt = crypto::new_salt()?;
    let keys = crypto::derive(pass, &salt, crypto::ROUNDS);
    let v = relay::Vault { salt: crypto::b64(&salt), rounds: crypto::ROUNDS, check: crypto::check(&keys)? };
    relay::set_vault(token, &v, replace).map_err(|e| e.words())?;
    sync::reset_sent();
    Ok(keys)
}

impl Inbox {
    /// Hand the saving thread the agents with a session, every tick while Remote is on.
    pub(in crate::inbox) fn cloud_feed(&self) {
        if !self.cloud.saved.on {
            return;
        }
        let sources = self
            .fleet
            .seats
            .iter()
            .filter_map(|s| {
                Some(sync::Source {
                    cli: s.spec.cli.to_ascii_lowercase(),
                    session: s.spec.session_id.clone()?,
                    run_dir: s.spec.run_dir().to_string(),
                    agent: s.spec.name.clone(),
                    project: folder_name(&s.spec.cwd),
                    started: s.spec.started_ms,
                })
            })
            .collect();
        sync::set_sources(sources);
    }

    pub(super) fn cloud_open_settings(&mut self) {
        if !self.cloud.loaded {
            self.cloud.saved = secret::load();
            self.cloud.loaded = true;
        }
        self.cloud.form = None;
        self.cloud.note = None;
    }

    fn cloud_store(&mut self, cx: &mut Context<Self>) {
        if let Err(e) = secret::save(&self.cloud.saved) {
            self.cloud.note = Some(format!("Couldn't keep the Remote settings: {e}"));
        }
        cx.notify();
    }

    fn cloud_toggle(&mut self, cx: &mut Context<Self>) {
        if self.cloud.busy {
            return;
        }
        self.cloud.saved.on = !self.cloud.saved.on;
        self.cloud_store(cx);
        if self.cloud.saved.on && self.cloud.saved.token.is_none() {
            self.cloud_sign_in(cx);
        }
    }

    fn cloud_sign_in(&mut self, cx: &mut Context<Self>) {
        let login = match relay::Login::start(&self.cloud.saved.machine) {
            Ok(l) => l,
            Err(e) => {
                self.cloud.note = Some(e);
                return cx.notify();
            }
        };
        cx.open_url(&login.url);
        self.cloud.busy = true;
        self.cloud.note = Some("Finish signing in with GitHub in your browser.".into());
        let wait = cx.background_spawn(async move { login.wait(std::time::Duration::from_secs(300)) });
        cx.spawn(async move |this, cx| {
            let got = wait.await;
            let _ = this.update(cx, |this, cx| {
                this.cloud.busy = false;
                match got {
                    Ok(token) => {
                        this.cloud.saved.token = Some(token);
                        this.cloud.note = Some("Signed in.".into());
                    }
                    Err(e) => this.cloud.note = Some(e),
                }
                this.cloud_store(cx);
            });
        })
        .detach();
        cx.notify();
    }

    fn cloud_open_form(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let first = cx.new(|cx| InputState::new(window, cx).placeholder("Passphrase").masked(true));
        let again = cx.new(|cx| InputState::new(window, cx).placeholder("Same passphrase again").masked(true));
        first.update(cx, |i, cx| i.focus(window, cx));
        self.cloud.form = Some(PassForm { first, again, error: None });
        cx.notify();
    }

    fn cloud_save_pass(&mut self, replace: bool, cx: &mut Context<Self>) {
        let Some(token) = self.cloud.saved.token.clone() else { return };
        let Some(f) = &mut self.cloud.form else { return };
        if self.cloud.busy {
            return;
        }
        let (a, b) = (f.first.read(cx).value().to_string(), f.again.read(cx).value().to_string());
        if let Some(p) = pass_problem(&a, &b) {
            f.error = Some(p);
            return cx.notify();
        }
        self.cloud.busy = true;
        f.error = None;
        let work = cx.background_spawn(async move { vault_keys(&token, &a, replace) });
        cx.spawn(async move |this, cx| {
            let got = work.await;
            let _ = this.update(cx, |this, cx| {
                this.cloud.busy = false;
                match got {
                    Ok(keys) => {
                        this.cloud.saved.set_keys(Some(&keys));
                        this.cloud.form = None;
                        this.cloud.note = Some("Passphrase set. Sessions are saved from now on.".into());
                        this.cloud_store(cx);
                    }
                    Err(e) => {
                        if let Some(f) = &mut this.cloud.form {
                            f.error = Some(e);
                        }
                        cx.notify();
                    }
                }
            });
        })
        .detach();
        cx.notify();
    }

    pub(super) fn cloud_section(&self, cx: &mut Context<Self>) -> Div {
        let c = &self.cloud;
        let st = sync::status();
        let status = match (&c.saved.token, c.saved.keys(), &st.error) {
            _ if !c.saved.on => "Off. Turn on to read your sessions from any device at remote.zoldify.com.".to_string(),
            (None, _, _) => "Sign in with GitHub to start.".to_string(),
            (_, None, _) => "Set a passphrase to start.".to_string(),
            (_, _, Some(e)) => e.clone(),
            _ if st.last_ok_ms.is_some() => format!("{} session{} saved, sealed with your passphrase.", st.saved, if st.saved == 1 { "" } else { "s" }),
            _ => "Saving.".to_string(),
        };
        let account = row("GitHub", if c.saved.token.is_some() { "Signed in." } else { "Not signed in." })
            .child(button("cloud-sign-in", if c.saved.token.is_some() { "Sign in again" } else { "Sign in" }).on_click(cx.listener(|this, _, _, cx| this.cloud_sign_in(cx))));
        let has_keys = c.saved.keys().is_some();
        let pass = row("Passphrase", "Locks every saved session. The server cannot read them, and a forgotten passphrase cannot be recovered.")
            .when(c.saved.token.is_some(), |r| {
                r.child(button("cloud-pass", if has_keys { "Change" } else { "Set passphrase" }).on_click(cx.listener(|this, _, window, cx| this.cloud_open_form(window, cx))))
            });
        let form = c.form.as_ref().map(|f| {
            div()
                .flex()
                .flex_col()
                .gap(px(12.0))
                .py(px(13.0))
                .border_b_1()
                .border_color(hairline())
                .child(text(
                    if has_keys { "A new passphrase" } else { "The Remote passphrase" },
                    if has_keys { "Sessions saved with the old one are removed and saved again. Other devices must type the new one." } else { "Type it on another device to read your sessions there; it remembers it for 15 days. At least 10 characters." },
                ))
                .child(div().max_w(px(320.0)).child(Input::new(&f.first)))
                .child(div().max_w(px(320.0)).child(Input::new(&f.again)))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.0))
                        .child(div().flex_1().min_h(px(12.0)).text_size(px(12.0)).text_color(color(0xf87171)).children(f.error.clone()))
                        .child(button("cloud-pass-cancel", "Cancel").on_click(cx.listener(|this, _, _, cx| {
                            this.cloud.form = None;
                            cx.notify();
                        })))
                        .child(button("cloud-pass-save", if c.busy { "Working…" } else { "Save passphrase" }).on_click(cx.listener(move |this, _, _, cx| this.cloud_save_pass(has_keys, cx)))),
                )
        });
        section("Save sessions to Remote")
            .child(toggle("cloud-on", c.saved.on, "Save sessions to Remote", status).on_click(cx.listener(|this, _, _, cx| this.cloud_toggle(cx))))
            .when(c.saved.on, |s| s.child(account).child(pass).children(form))
            .children(c.note.clone().map(|n| div().py(px(8.0)).text_size(px(12.0)).text_color(color(INK_3)).child(n)))
    }
}

#[cfg(test)]
mod tests {
    use super::{folder_name, pass_problem};

    #[test]
    fn names_a_project_by_its_folder() {
        assert_eq!(folder_name("D:\\work\\bytehive\\"), "bytehive");
        assert_eq!(folder_name("D:/a/b"), "b");
    }

    #[test]
    fn a_passphrase_is_long_enough_and_typed_the_same_twice() {
        assert!(pass_problem("short", "short").is_some());
        assert!(pass_problem("long enough one", "long enough two").is_some());
        assert_eq!(pass_problem("long enough one", "long enough one"), None);
    }
}
