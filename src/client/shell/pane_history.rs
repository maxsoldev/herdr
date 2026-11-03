//! Browser-style pane focus history for the TUI client.

/// Upper bound on remembered panes; the oldest entries drop first.
const LIMIT: usize = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PaneHistoryStep {
    Back,
    Forward,
}

/// Visited panes plus a cursor, like a browser tab's history.
#[derive(Debug, Default)]
pub(crate) struct PaneHistory {
    entries: Vec<String>,
    index: usize,
    /// A back/forward focus request is waiting for the server to confirm it.
    in_flight: bool,
}

impl PaneHistory {
    /// Records an observed focus move. Landing on the cursor entry confirms a
    /// back/forward step; landing on another history entry while a step is in
    /// flight is an intermediate result of rapid presses. Anything else is a
    /// new visit that drops the forward entries.
    pub(crate) fn record(&mut self, from: &str, to: &str) {
        if self.entries.get(self.index).map(String::as_str) == Some(to) {
            self.in_flight = false;
            return;
        }
        if self.in_flight && self.entries.iter().any(|entry| entry == to) {
            return;
        }
        self.in_flight = false;
        if self.entries.get(self.index).map(String::as_str) != Some(from) {
            self.push(from);
        }
        self.push(to);
    }

    /// Moves the cursor to the nearest live entry in `step` direction that is
    /// not `focused`, and returns it as the pane to focus.
    pub(crate) fn step(
        &mut self,
        step: PaneHistoryStep,
        focused: Option<&str>,
        exists: impl Fn(&str) -> bool,
    ) -> Option<String> {
        let usable = |entry: &String| Some(entry.as_str()) != focused && exists(entry);
        let target = match step {
            PaneHistoryStep::Back => (0..self.index).rev().find(|&i| usable(&self.entries[i])),
            PaneHistoryStep::Forward => {
                (self.index + 1..self.entries.len()).find(|&i| usable(&self.entries[i]))
            }
        }?;
        self.index = target;
        self.in_flight = true;
        Some(self.entries[target].clone())
    }

    pub(crate) fn clear(&mut self) {
        *self = Self::default();
    }

    fn push(&mut self, id: &str) {
        self.entries.truncate(self.index + 1);
        if self.entries.last().map(String::as_str) != Some(id) {
            self.entries.push(id.to_owned());
        }
        if self.entries.len() > LIMIT {
            self.entries.remove(0);
        }
        self.index = self.entries.len() - 1;
    }
}

#[cfg(test)]
mod tests {
    use super::{PaneHistory, PaneHistoryStep::*};

    fn visit(history: &mut PaneHistory, path: &[&str]) {
        for pair in path.windows(2) {
            history.record(pair[0], pair[1]);
        }
    }

    #[test]
    fn back_and_forward_walk_the_visit_order() {
        let mut h = PaneHistory::default();
        visit(&mut h, &["a", "b", "c"]);
        assert_eq!(h.step(Back, Some("c"), |_| true).as_deref(), Some("b"));
        h.record("c", "b");
        assert_eq!(h.step(Back, Some("b"), |_| true).as_deref(), Some("a"));
        h.record("b", "a");
        assert_eq!(h.step(Back, Some("a"), |_| true), None);
        assert_eq!(h.step(Forward, Some("a"), |_| true).as_deref(), Some("b"));
        h.record("a", "b");
        assert_eq!(h.step(Forward, Some("b"), |_| true).as_deref(), Some("c"));
        h.record("b", "c");
        assert_eq!(h.step(Forward, Some("c"), |_| true), None);
    }

    #[test]
    fn new_visit_after_back_drops_forward_entries() {
        let mut h = PaneHistory::default();
        visit(&mut h, &["a", "b", "c"]);
        h.step(Back, Some("c"), |_| true);
        h.record("c", "b");
        h.record("b", "d");
        assert_eq!(h.step(Forward, Some("d"), |_| true), None);
        assert_eq!(h.step(Back, Some("d"), |_| true).as_deref(), Some("b"));
    }

    #[test]
    fn rapid_back_presses_survive_intermediate_focus_updates() {
        let mut h = PaneHistory::default();
        visit(&mut h, &["a", "b", "c"]);
        h.step(Back, Some("c"), |_| true);
        h.step(Back, Some("c"), |_| true);
        h.record("c", "b");
        h.record("b", "a");
        assert_eq!(h.step(Forward, Some("a"), |_| true).as_deref(), Some("b"));
    }

    #[test]
    fn closed_panes_are_skipped() {
        let mut h = PaneHistory::default();
        visit(&mut h, &["a", "b", "c"]);
        assert_eq!(
            h.step(Back, Some("c"), |id| id != "b").as_deref(),
            Some("a")
        );
    }
}
