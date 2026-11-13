//! Starting the Inbox: its timers, the fleet and settings it opens with, the one-time
//! import from the app before this one, and the tray and browser it starts.

use super::*;

impl Inbox {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let ticker = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(TICK).await;
                if this.update(cx, |this, cx| this.tick(cx)).is_err() {
                    break;
                }
            }
        });
        // A few seconds after start, never in the way of CLIs starting, then
        // every few minutes.
        let plans = cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_millis(plan::FIRST_MS)).await;
            loop {
                if this.update(cx, |this, cx| this.ask_plans(cx)).is_err() {
                    break;
                }
                cx.background_executor().timer(Duration::from_millis(plan::EVERY_MS)).await;
            }
        });
        // Every few seconds while a Claude agent runs: the session it is in now.
        let follow = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_millis(crate::follow::CHECK_MS)).await;
                let Ok(any) = this.update(cx, |this, cx| !this.fleet.claude_runs(cx).is_empty()) else { break };
                if !any {
                    continue;
                }
                let live = cx.background_executor().spawn(async { agent_core::claude::live_sessions() }).await;
                if this.update(cx, |this, cx| this.fleet.follow(&live, cx)).is_err() {
                    break;
                }
            }
        });
        let mut prefs = Prefs::load();
        // Once: the fleet and settings of the app before this one.
        let imported = crate::import::take(&mut prefs);
        crate::terminal::set_font_size(prefs.font_size);
        // Settings → Reopen my projects on start.
        let mut fleet = if prefs.restore { Fleet::load() } else { Fleet::default() };
        let came = imported.map(|found| {
            let n = fleet.adopt(found.specs.clone());
            crate::import::finish(&found, n);
            n
        });
        let mut inbox = Self {
            home: fleet.seats.is_empty(),
            compose: None,
            popup: None,
            settings: None,
            sessions: None,
            toast: None,
            changes: None,
            kept: HashMap::new(),
            history: Histories::default(),
            files: None,
            bridge: None,
            memory: None,
            usage_view: None,
            schedules: None,
            scheduler: None,
            board: None,
            conductor: Default::default(),
            company: Default::default(),
            limits: limits::LimitGuard::default(),
            updates: updates::Updates::default(),
            notices: notices::Notices::default(),
            moves: moves::Moves::default(),
            grid: grid::Grid::default(),
            tips: tips::Tips::default(),
            browser: browser::BrowserWatch::default(),
            remote: remote::Remote::new(),
            cloud: settings::cloud::CloudUi::load(),
            tray: crate::tray::Tray::new(),
            plans: Default::default(),
            uses: HashMap::new(),
            _plans: plans,
            fleet,
            prefs,
            sheet: None,
            jobs: Vec::new(),
            typing: false,
            menu: None,
            palette: None,
            removing: None,
            folded: HashSet::new(),
            since: HashMap::new(),
            worked: HashSet::new(),
            _ticker: ticker,
            _follow: follow,
        };
        inbox.show_tray();
        inbox.start_browser(cx);
        inbox.boot_remote(cx);
        if let Some(n) = came {
            let agents = if n == 1 { "1 agent".to_string() } else { format!("{n} agents") };
            inbox.toast_for(format!("Brought over {agents} and your settings from ByteHive. Resume an agent to carry on its conversation."), 15_000, cx);
        }
        inbox
    }
}
