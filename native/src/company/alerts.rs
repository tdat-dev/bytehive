//! What the owner has to hear (alerts.ts, and the Alert of watch.ts). A standing
//! alert is what no watch will bring up again: a file that won't read, a member
//! who starts blank. It stays until it is dismissed or stops being true.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Warn,
    Stop,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Alert {
    pub key: String,
    pub job: Option<String>,
    pub person_id: Option<String>,
    pub text: String,
    pub level: Level,
    /// Stays until dismissed.
    pub standing: bool,
}

/// The standing alerts, in the order they were first raised. Whoever owns it (the
/// engine) hands it to the pane or the store that has something to raise.
#[derive(Debug, Default)]
pub struct Standing {
    alerts: Vec<Alert>,
    /// Goes up on every change, so a view can tell it has to redraw.
    pub changes: u64,
}

impl Standing {
    pub fn list(&self) -> &[Alert] {
        &self.alerts
    }

    /// Raises it, or puts the new text in place of the one under the same key.
    pub fn raise(&mut self, key: &str, text: &str, person_id: Option<&str>) {
        let alert = Alert {
            key: key.into(),
            job: None,
            person_id: person_id.map(Into::into),
            text: text.into(),
            level: Level::Warn,
            standing: true,
        };
        match self.alerts.iter_mut().find(|a| a.key == key) {
            Some(old) => *old = alert,
            None => self.alerts.push(alert),
        }
        self.changes += 1;
    }

    /// Whether there was one under that key.
    pub fn clear(&mut self, key: &str) -> bool {
        let before = self.alerts.len();
        self.alerts.retain(|a| a.key != key);
        let gone = self.alerts.len() != before;
        if gone {
            self.changes += 1;
        }
        gone
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_raised_alert_stands_as_a_warning_until_cleared() {
        let mut s = Standing::default();
        s.raise("job-file-J-0001.json", "unreadable", None);
        assert_eq!(s.list(), [Alert { key: "job-file-J-0001.json".into(), job: None, person_id: None, text: "unreadable".into(), level: Level::Warn, standing: true }]);
        assert!(s.clear("job-file-J-0001.json"));
        assert!(s.list().is_empty());
    }

    #[test]
    fn raising_a_key_again_keeps_its_place_with_the_new_words() {
        let mut s = Standing::default();
        s.raise("a", "one", None);
        s.raise("b", "two", Some("p1"));
        s.raise("a", "three", None);
        let got: Vec<(&str, &str)> = s.list().iter().map(|a| (a.key.as_str(), a.text.as_str())).collect();
        assert_eq!(got, [("a", "three"), ("b", "two")]);
        assert_eq!(s.list()[1].person_id.as_deref(), Some("p1"));
    }

    #[test]
    fn only_a_real_change_counts() {
        let mut s = Standing::default();
        assert!(!s.clear("nothing"));
        assert_eq!(s.changes, 0);
        s.raise("a", "x", None);
        s.clear("a");
        assert_eq!(s.changes, 2);
    }
}
