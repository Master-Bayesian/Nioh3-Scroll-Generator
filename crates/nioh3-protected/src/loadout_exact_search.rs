//! Run the proven required-set search within a protected save job.
use crate::error::HostError;
use nioh3_loadout::{
    engine::{Backpack, Engine, Work},
    search::{
        Backend, Counts, EnumerationOrder, ExactSweep, Report, Request, ScorerKind, SetPruning,
    },
    search_control::{
        SearchBackend, SearchInterrupted, SearchObserver, SearchProgress, SearchStage,
    },
};
use std::time::Instant;

pub(super) struct Completed {
    pub(super) report: Report,
    pub(super) work: Work,
    pub(super) planned: u64,
    pub(super) excluded: u64,
    pub(super) execution_receipt: ExecutionReceipt,
}

/// Actual completed sweep work, excluding startup probes, rank-detail rechecks
/// and report serialization. Contacts and outfits are different units.
#[derive(serde::Serialize)]
pub(super) struct ExecutionReceipt {
    schema: &'static str,
    scope: &'static str,
    backend: &'static str,
    planned: u64,
    completed: u64,
    cpu_executed_candidates: u64,
    cpu_certified_candidates: u64,
    completed_contacts: u64,
    submitted_contacts: u64,
    native_calls: u64,
    failed_batches: u64,
    reuse_enabled: bool,
}

fn execution_receipt(
    backend: &Backend,
    planned: u64,
    completed: u64,
    performance: nioh3_loadout::search::ExactPerformance,
) -> ExecutionReceipt {
    let device = performance
        .device
        .map(|device| device.reuse)
        .unwrap_or_default();
    ExecutionReceipt {
        schema: "nioh3-loadout-execution/v1",
        scope: "prepared_exact_sweep",
        backend: match backend {
            Backend::Card => "card",
            Backend::Cpu(_) => "cpu",
        },
        planned,
        completed,
        cpu_executed_candidates: performance.cpu_executed_candidates,
        cpu_certified_candidates: performance.cpu_certified_candidates,
        completed_contacts: device.completed_contacts,
        submitted_contacts: device.submitted_contacts,
        native_calls: device.native_calls,
        failed_batches: device.failed_batches,
        reuse_enabled: device.enabled,
    }
}

pub(super) fn run(
    engine: &Engine,
    pack: &Backpack,
    request: Request,
    swaps: &[(u32, usize)],
    observer: Option<&dyn SearchObserver>,
) -> Result<Completed, HostError> {
    let started = Instant::now();
    let check_cancelled = || {
        if observer.is_some_and(SearchObserver::cancelled) {
            return Err(HostError::rejected(
                SearchInterrupted::Cancelled.to_string(),
            ));
        }
        Ok(())
    };
    check_cancelled()?;
    let open = |kind| {
        ExactSweep::open(
            engine,
            pack,
            request.clone(),
            kind,
            EnumerationOrder::Spread,
            swaps,
            SetPruning::Proven,
        )
    };
    let prefer_cuda = engine.supports_prepared_device()
        && std::env::var_os("NIOH3_LOADOUT_GPU").is_none_or(|selected| selected != "0");
    let (mut sweep, backend) = if prefer_cuda {
        match open(ScorerKind::Cuda) {
            Ok(sweep) => (sweep, Backend::Card),
            Err(error) => (
                open(ScorerKind::Cpu).map_err(HostError::rejected)?,
                Backend::Cpu(error),
            ),
        }
    } else {
        (
            open(ScorerKind::Cpu).map_err(HostError::rejected)?,
            Backend::Cpu("prepared CUDA disabled or unavailable".to_owned()),
        )
    };
    let progress_backend = match &backend {
        Backend::Card => SearchBackend::Card,
        Backend::Cpu(_) => SearchBackend::Cpu,
    };
    let mut state = sweep.start();
    let planned = state.planned;
    let publish = |stage, completed| {
        if let Some(observer) = observer {
            observer.progress(SearchProgress::new(
                stage,
                progress_backend,
                completed,
                Some(planned),
            ));
        }
    };
    publish(SearchStage::Evaluation, 0);
    while state.completed < planned {
        check_cancelled()?;
        sweep
            .advance_controlled(&mut state, 2048, &|| {
                observer.is_some_and(SearchObserver::cancelled)
            })
            .map_err(HostError::rejected)?;
        check_cancelled()?;
        publish(SearchStage::Evaluation, state.completed);
    }
    check_cancelled()?;
    publish(SearchStage::Finalizing, state.completed);
    let mut work = sweep.work().map_err(HostError::rejected)?;
    let best = sweep
        .best(&state)
        .map_err(HostError::rejected)?
        .into_iter()
        .map(|scored| {
            let facts = engine.facts(&scored.candidate, pack, &mut work);
            (scored, facts)
        })
        .collect();
    let missed = state
        .unanswered_samples
        .iter()
        .filter_map(|&ordinal| sweep.candidate_at(ordinal))
        .collect();
    check_cancelled()?;
    let execution_receipt =
        execution_receipt(&backend, planned, state.completed, sweep.performance());
    Ok(Completed {
        work,
        planned,
        excluded: state.excluded_by_set_bound,
        execution_receipt,
        report: Report {
            failure: None,
            best,
            backend,
            counts: Counts {
                answered: state.answered,
                refused: state.refused,
                no_path: state.no_path,
                outside_limits: state.outside_limits,
                certified: state.certified,
                ..Counts::default()
            },
            searched: state.completed,
            full_size: state.requested,
            source_uncomputed: 0,
            polish_rounds: 0,
            seconds: started.elapsed().as_secs_f64(),
            missed,
            complete: state.complete(),
        },
    })
}

#[cfg(test)]
mod execution_receipt_tests {
    use super::*;
    use serde_json::{json, Value};

    #[test]
    fn sweep_execution_receipt_preserves_counters_and_strict_wire_shape(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut performance = nioh3_loadout::search::ExactPerformance {
            cpu_executed_candidates: 12,
            cpu_certified_candidates: 88,
            ..Default::default()
        };
        let cpu = serde_json::to_value(execution_receipt(
            &Backend::Cpu("selected".to_owned()),
            100,
            100,
            performance.clone(),
        ))?;
        assert_eq!(cpu["cpu_executed_candidates"], 12);
        assert_eq!(cpu["cpu_certified_candidates"], 88);
        assert_eq!(cpu["native_calls"], 0);
        assert_eq!(cpu["completed_contacts"], 0);
        let mut device = nioh3_loadout::gpu::PreparedDeviceStats::default();
        device.reuse.native_calls = 3;
        device.reuse.submitted_contacts = 200;
        device.reuse.completed_contacts = 200;
        performance.device = Some(device);
        performance.cpu_executed_candidates = 0;
        performance.cpu_certified_candidates = 0;
        let card = serde_json::to_value(execution_receipt(&Backend::Card, 100, 100, performance))?;
        assert_eq!(card["backend"], "card");
        assert_eq!(card["native_calls"], 3);
        // Two contacts per candidate must not be mislabeled as 200 outfits.
        assert_eq!(card["completed"], 100);
        assert_eq!(card["completed_contacts"], 200);
        let schema: Value = serde_json::from_str(include_str!(
            "../../../packages/contracts/protected-response.schema.json"
        ))?;
        let validator =
            jsonschema::validator_for(&schema["definitions"]["LoadoutExecutionReceipt"])?;
        assert!(validator.is_valid(&cpu));
        assert!(validator.is_valid(&card));
        let mut bad = card.clone();
        bad["native_calls"] = json!(-1);
        assert!(!validator.is_valid(&bad));
        let mut bad = card.clone();
        bad["unexpected"] = json!(1);
        assert!(!validator.is_valid(&bad));
        let mut bad = card;
        bad.as_object_mut()
            .ok_or("expected object")?
            .remove("native_calls");
        assert!(!validator.is_valid(&bad));
        Ok(())
    }
}
