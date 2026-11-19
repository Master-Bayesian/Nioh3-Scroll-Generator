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
            .advance(&mut state, 2048)
            .map_err(HostError::rejected)?;
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
    Ok(Completed {
        work,
        planned,
        excluded: state.excluded_by_set_bound,
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
