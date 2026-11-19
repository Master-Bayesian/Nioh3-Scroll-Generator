//! Cooperative search ownership and progress; cancellation never carries a ranking.
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Mutex;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SearchStage {
    Planning,
    Verification,
    Evaluation,
    Retrying,
    Polishing,
    Finalizing,
}
impl SearchStage {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Planning => "planning",
            Self::Verification => "verification",
            Self::Evaluation => "evaluation",
            Self::Retrying => "retrying",
            Self::Polishing => "polishing",
            Self::Finalizing => "finalizing",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SearchBackend {
    Unselected,
    Cpu,
    Card,
}
impl SearchBackend {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Unselected => "unselected",
            Self::Cpu => "cpu",
            Self::Card => "card",
        }
    }
}
/// Completed counts contain only settled candidates in this attempt. Device work
/// in an unfinished/retried batch is reported separately and never certifies a rank.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SearchProgress {
    pub stage: SearchStage,
    pub backend: SearchBackend,
    pub completed: u64,
    pub total: Option<u64>,
    pub batch_completed: u64,
    pub batch_total: Option<u64>,
    pub batch_index: u64,
    pub attempt: u32,
}
impl SearchProgress {
    pub fn new(
        stage: SearchStage,
        backend: SearchBackend,
        completed: u64,
        total: Option<u64>,
    ) -> Self {
        Self {
            stage,
            backend,
            completed,
            total,
            batch_completed: 0,
            batch_total: None,
            batch_index: 0,
            attempt: 0,
        }
    }
}
/// Called synchronously on the owning search threads. A device callback never
/// exposes a Rust atomic as a non-atomic foreign-language cancel word.
pub trait SearchObserver: Sync {
    fn cancelled(&self) -> bool;
    fn progress(&self, progress: SearchProgress);
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SearchInterrupted {
    Cancelled,
    ObserverFailed,
}
impl std::fmt::Display for SearchInterrupted {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Cancelled => "LOADOUT_SEARCH_CANCELLED",
            Self::ObserverFailed => "LOADOUT_SEARCH_OBSERVER_FAILED",
        })
    }
}
impl std::error::Error for SearchInterrupted {}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RunState {
    Running,
    Cancelled,
    ObserverFailed,
}
struct Published {
    last: Option<SearchProgress>,
}
pub(crate) struct RunControl<'a> {
    observer: &'a dyn SearchObserver,
    state: Mutex<RunState>,
    published: Mutex<Published>,
}
impl<'a> RunControl<'a> {
    pub(crate) fn new(observer: &'a dyn SearchObserver) -> Self {
        Self {
            observer,
            state: Mutex::new(RunState::Running),
            published: Mutex::new(Published { last: None }),
        }
    }
    fn record(&self, outcome: RunState) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if *state == RunState::Running {
            *state = outcome;
        }
    }
    pub(crate) fn checkpoint(&self) -> bool {
        if *self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            != RunState::Running
        {
            return false;
        }
        match catch_unwind(AssertUnwindSafe(|| self.observer.cancelled())) {
            Ok(false) => true,
            Ok(true) => {
                self.record(RunState::Cancelled);
                false
            }
            Err(_) => {
                self.record(RunState::ObserverFailed);
                false
            }
        }
    }
    pub(crate) fn observer_failed(&self) {
        self.record(RunState::ObserverFailed);
    }
    pub(crate) fn report(&self, mut progress: SearchProgress) -> bool {
        if !self.checkpoint() {
            return false;
        }
        let mut published = self
            .published
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(old) = published.last.filter(|old| {
            old.stage == progress.stage
                && old.backend == progress.backend
                && old.attempt == progress.attempt
        }) {
            progress.completed = progress.completed.max(old.completed);
        }
        {
            match catch_unwind(AssertUnwindSafe(|| self.observer.progress(progress))) {
                Ok(()) => {
                    published.last = Some(progress);
                }
                Err(_) => {
                    self.record(RunState::ObserverFailed);
                    return false;
                }
            }
        }
        self.checkpoint()
    }
    pub(crate) fn outcome(&self) -> Result<(), SearchInterrupted> {
        self.checkpoint();
        match *self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
        {
            RunState::Running => Ok(()),
            RunState::Cancelled => Err(SearchInterrupted::Cancelled),
            RunState::ObserverFailed => Err(SearchInterrupted::ObserverFailed),
        }
    }
}
