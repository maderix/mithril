use std::sync::{Condvar, Mutex};
use std::time::Duration;

struct State {
    generation: usize,
    waiting: bool,
    ready: bool,
}

pub struct Signal {
    state: Mutex<State>,
    wakeup: Condvar,
}

pub struct Ticket<'a> {
    signal: &'a Signal,
    token: usize,
}

impl Signal {
    pub const fn new() -> Self {
        Self { state: Mutex::new(State { generation: 0, waiting: false, ready: false }), wakeup: Condvar::new() }
    }

    pub fn register(&self) -> Result<Ticket<'_>, String> {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.waiting { return Err("a completion wait is already active".into()); }
        state.generation = state.generation.checked_add(1).ok_or("completion generations exhausted")?;
        state.waiting = true;
        state.ready = false;
        Ok(Ticket { signal: self, token: state.generation })
    }

    pub fn complete(&self, token: usize) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.waiting && state.generation == token {
            state.ready = true;
            self.wakeup.notify_one();
        }
    }
}

impl Ticket<'_> {
    pub fn token(&self) -> usize { self.token }

    pub fn wait(&self, timeout: Duration) -> bool {
        let state = self.signal.state.lock().unwrap_or_else(|e| e.into_inner());
        let (state, _) = self.signal.wakeup.wait_timeout_while(state, timeout,
            |state| state.generation == self.token && state.waiting && !state.ready)
            .unwrap_or_else(|e| e.into_inner());
        state.generation == self.token && state.waiting && state.ready
    }
}

impl Drop for Ticket<'_> {
    fn drop(&mut self) {
        let mut state = self.signal.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.generation == self.token { state.waiting = false; }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completion_handles_early_late_and_duplicate_notifications() {
        let signal = Signal::new();
        let first = signal.register().unwrap();
        assert!(!first.wait(Duration::ZERO));
        signal.complete(first.token());
        signal.complete(first.token());
        assert!(first.wait(Duration::ZERO));
        let old = first.token();
        drop(first);
        signal.complete(old);
        let second = signal.register().unwrap();
        assert_ne!(second.token(), old);
        signal.complete(old);
        assert!(!second.wait(Duration::ZERO));
        signal.complete(second.token());
        assert!(second.wait(Duration::ZERO));
    }

    #[test]
    fn abandoned_waits_do_not_retain_state_or_wake_later_generations() {
        let signal = Signal::new();
        for _ in 0..1000 {
            let expired = signal.register().unwrap().token();
            let live = signal.register().unwrap();
            assert!(signal.register().is_err());
            signal.complete(expired);
            assert!(!live.wait(Duration::ZERO));
            signal.complete(live.token());
            assert!(live.wait(Duration::ZERO));
        }
    }

    #[test]
    fn notification_wakes_a_blocked_waiter() {
        let signal = Signal::new();
        let ticket = signal.register().unwrap();
        std::thread::scope(|scope| {
            scope.spawn(|| {
                std::thread::sleep(Duration::from_millis(10));
                signal.complete(ticket.token());
            });
            assert!(ticket.wait(Duration::from_secs(1)));
        });
    }

    #[test]
    fn cancellation_racing_a_callback_cannot_complete_the_next_wait() {
        let signal = Signal::new();
        for _ in 0..100 {
            let cancelled = signal.register().unwrap();
            let token = cancelled.token();
            std::thread::scope(|scope| {
                scope.spawn(|| signal.complete(token));
                drop(cancelled);
                let next = signal.register().unwrap();
                assert!(!next.wait(Duration::ZERO));
                signal.complete(next.token());
                assert!(next.wait(Duration::ZERO));
            });
        }
    }

    #[test]
    fn tokens_cannot_wrap_and_alias_an_old_callback() {
        let signal = Signal::new();
        signal.state.lock().unwrap().generation = usize::MAX - 1;
        let last = signal.register().unwrap();
        assert_eq!(last.token(), usize::MAX);
        drop(last);
        assert!(signal.register().is_err());
    }
}
