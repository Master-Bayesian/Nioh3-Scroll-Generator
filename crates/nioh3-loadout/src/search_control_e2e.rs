//! Current-context E2E of real Engine/Search/CUDA control. Duplicate options are
//! a numerical/ownership fixture, not a claim about 5,000 owned legal items.
use super::*;
use crate::context::LoadoutContext;
use crate::search_control::{
    SearchBackend, SearchInterrupted, SearchObserver, SearchProgress, SearchStage,
};
use std::sync::atomic::AtomicBool;

#[derive(Clone, Copy, Debug)]
enum StopAt {
    Never,
    Prefix,
    Finalizing,
    ObserverFault,
}
struct Probe {
    stop: StopAt,
    route: String,
    cancelled: AtomicBool,
    events: Mutex<Vec<SearchProgress>>,
}
impl Probe {
    fn new(stop: StopAt, route: &str) -> Self {
        Self {
            stop,
            route: route.into(),
            cancelled: AtomicBool::new(false),
            events: Mutex::new(Vec::new()),
        }
    }
    fn events(&self) -> Vec<SearchProgress> {
        self.events
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }
}
impl SearchObserver for Probe {
    fn cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
    fn progress(&self, p: SearchProgress) {
        self.events
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .push(p);
        let prefix = match self.route.as_str() {
            "cpu" => {
                p.stage == SearchStage::Evaluation
                    && p.backend == SearchBackend::Cpu
                    && p.completed >= 2048
            }
            "legacy" => {
                p.stage == SearchStage::Evaluation
                    && p.backend == SearchBackend::Card
                    && p.completed >= 512
            }
            "device" => {
                p.stage == SearchStage::Evaluation
                    && p.backend == SearchBackend::Card
                    && p.batch_completed >= 4096
            }
            other => panic!("Unknown route {other}"),
        };
        match self.stop {
            StopAt::Never => {}
            StopAt::Prefix if prefix => self.cancelled.store(true, Ordering::SeqCst),
            StopAt::Finalizing if p.stage == SearchStage::Finalizing => {
                self.cancelled.store(true, Ordering::SeqCst)
            }
            StopAt::ObserverFault if prefix => {
                panic!("deliberate observer fault at a real search checkpoint")
            }
            StopAt::Prefix | StopAt::Finalizing | StopAt::ObserverFault => {}
        }
    }
}
fn identities(report: &Report) -> Vec<(u32, Candidate)> {
    report
        .best
        .iter()
        .map(|(score, _)| (score.bits, score.candidate))
        .collect()
}
#[test]
#[ignore = "Requires the explicit current-context CPU/CUDA fixtures and serialized host execution"]
fn current_engine_search_control_cancel_cleanup_and_resume() {
    let root = std::path::PathBuf::from(
        std::env::var_os("NIOH3_CONTROL_AUDIT_ROOT").expect("current fixture root"),
    );
    let route = std::env::var("NIOH3_CONTROL_ROUTE").expect("cpu/device/legacy route");
    let bundle = match route.as_str() {
        "cpu" | "device" => "zero-bundle",
        "legacy" => "legacy-zero-bundle",
        other => panic!("Unknown route {other}"),
    };
    let source: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("CURRENT_CANDIDATE_FIELDS.json")).unwrap())
            .unwrap();
    let bytes: Vec<u8> = serde_json::from_value(source["pool_rows"][0].clone()).unwrap();
    let row: [u8; crate::engine::ITEM] = bytes.try_into().unwrap();
    let context = LoadoutContext::load(&root.join("held-context")).unwrap();
    let engine = Engine::open_with_context(&root.join(bundle), &context).unwrap();
    let pack = Backpack::from_slots(&(0..5000).map(|_| (4, row)).collect::<Vec<_>>()).unwrap();
    let request = Request {
        sets: Vec::new(),
        limits: Limits::NONE,
        move_points: false,
        threads: 1,
    };
    let search = Search::new(&engine, &pack);
    let baseline = search.run(&request);
    assert!(baseline.complete);
    assert_eq!(baseline.searched, 5000);
    assert_eq!(baseline.counts.answered, 5000);
    assert_eq!(baseline.counts.no_path + baseline.counts.refused, 0);
    match (&baseline.backend, route.as_str()) {
        (Backend::Cpu(_), "cpu") | (Backend::Card, "device" | "legacy") => {}
        other => panic!("Wrong execution backend: {other:?}"),
    }
    let never = Probe::new(StopAt::Never, &route);
    let observed = search
        .run_limited_by_observed(&request, |_| 10_000, &never)
        .unwrap()
        .unwrap()
        .1;
    assert_eq!(identities(&observed), identities(&baseline));
    assert_eq!(observed.counts.answered, baseline.counts.answered);
    let events = never.events();
    assert!(events.iter().any(|p| p.stage == SearchStage::Planning));
    assert!(events.iter().any(|p| p.stage == SearchStage::Finalizing));
    for p in &events {
        if let Some(total) = p.total {
            assert!(p.completed <= total, "{p:?}");
        }
        if let Some(total) = p.batch_total {
            assert!(p.batch_completed <= total, "{p:?}");
        }
    }
    let mut cases = Vec::new();
    for stop in [StopAt::Prefix, StopAt::Finalizing, StopAt::ObserverFault] {
        let probe = Probe::new(stop, &route);
        let outcome = search.run_limited_by_observed(&request, |_| 10_000, &probe);
        let expected = match stop {
            StopAt::ObserverFault => SearchInterrupted::ObserverFailed,
            StopAt::Prefix | StopAt::Finalizing => SearchInterrupted::Cancelled,
            StopAt::Never => unreachable!(),
        };
        match outcome {
            Err(error) => assert_eq!(error, expected),
            Ok(_) => panic!("Interrupted search published a ranking: {stop:?}"),
        }
        let events = probe.events();
        let device_prefix = events.iter().map(|p| p.batch_completed).max().unwrap_or(0);
        let settled = events.iter().map(|p| p.completed).max().unwrap_or(0);
        if matches!(stop, StopAt::Prefix) {
            if route == "device" {
                assert_eq!(device_prefix, 4096);
                assert!(device_prefix < 5000);
            }
            if route == "cpu" {
                assert_eq!(settled, 2048);
            }
            if route == "legacy" {
                assert_eq!(settled, 512);
            }
        }
        if route != "cpu" {
            assert!(
                !events.iter().any(|p| p.backend == SearchBackend::Cpu),
                "Cancellation started a CPU fallback"
            );
        }
        // Same context and handle must be immediately reusable after every stop/fault.
        let resumed = search.run(&request);
        assert_eq!(identities(&resumed), identities(&baseline));
        assert_eq!(resumed.counts.answered, 5000);
        cases.push(serde_json::json!({"stop":format!("{stop:?}"),"expected":expected.to_string(),"settled":settled,"device_prefix":device_prefix,"resumed_exactly":true,"ranking_published":false}));
    }
    let receipt = serde_json::json!({"passed":true,"route":route,"candidates":5000,"complete_bits_and_identities_equal":true,"cases":cases,"scope":"Actual current-context Engine/Search and device or legacy kernel, controlled duplicate zero-motion options; no game/save access or legality claim."});
    std::fs::write(
        root.join(format!("SEARCH_CONTROL_{}.json", route.to_uppercase())),
        serde_json::to_vec_pretty(&receipt).unwrap(),
    )
    .unwrap();
}
