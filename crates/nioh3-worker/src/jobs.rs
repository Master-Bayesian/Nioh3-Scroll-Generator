//! Single-owner offline search jobs.
//!
//! Ports `nioh3_scroll_editor/search_jobs.py`: one job per worker process runs
//! on a background thread while the framed main loop keeps answering status,
//! cancel and shutdown; every snapshot is a detached copy; the page checkpoint,
//! the private candidate records and the resume token are bound to the same
//! query/context/policy/continuation binding.

use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Instant;

use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::collector::{
    BatchRequest, CandidateSource, CollectorError, IntersectionReport, MaterializedCandidate,
    SearchCollector, SearchFactory, MISSING_COLLECTOR_MESSAGE,
};
use crate::grace_map::{self, GraceOutputMap, CATEGORY_TO_TYPE};
use crate::model::Candidate;
use crate::payload;
use crate::protocol::RequestError;
use crate::query::{OccurrenceScope, SearchQuery};

/// States that end a job.
pub const TERMINAL_STATES: [&str; 3] = ["completed", "cancelled", "failed"];

/// `SearchJobs`: at most 16 measured maps before the registry must be restarted.
pub const MAX_REGISTERED_MAPS: usize = 16;

/// The one job this worker retains, in wire shape.
#[derive(Debug, Clone)]
struct JobState {
    job_id: String,
    state: &'static str,
    sequence: u64,
    query_digest: String,
    context_digest: String,
    cursor: u64,
    start_cursor: u64,
    candidates: Vec<Value>,
    progress: Option<IntersectionReport>,
    stop_reason: Option<&'static str>,
    error: Option<(String, String)>,
    resume_token: Option<String>,
    elapsed_ms: u64,
}

/// A detached snapshot handed to the caller.
#[derive(Debug, Clone)]
pub struct JobView {
    pub job_id: String,
    pub state: &'static str,
    pub sequence: u64,
    pub query_digest: String,
    pub context_digest: String,
    pub cursor: u64,
    pub start_cursor: u64,
    pub candidates: Vec<Value>,
    pub progress: Option<IntersectionReport>,
    pub stop_reason: Option<&'static str>,
    pub error: Option<(String, String)>,
    pub resume_token: Option<String>,
    pub elapsed_ms: u64,
}

impl JobState {
    fn view(&self) -> JobView {
        JobView {
            job_id: self.job_id.clone(),
            state: self.state,
            sequence: self.sequence,
            query_digest: self.query_digest.clone(),
            context_digest: self.context_digest.clone(),
            cursor: self.cursor,
            start_cursor: self.start_cursor,
            candidates: self.candidates.clone(),
            progress: self.progress.clone(),
            stop_reason: self.stop_reason,
            error: self.error.clone(),
            resume_token: self.resume_token.clone(),
            elapsed_ms: self.elapsed_ms,
        }
    }
}

struct Inner {
    job: Option<JobState>,
    /// Private candidate records; never sent to a renderer, only exported.
    candidate_records: HashMap<String, Candidate>,
    level: u16,
    /// `SearchJobs.maps`: validated save-bound measured maps, keyed by the
    /// sha256 of the exact `cache_json` string the caller registered.
    maps: BTreeMap<String, GraceOutputMap>,
}

/// `search.start` arguments after schema validation.
#[derive(Debug, Clone)]
pub struct StartParams {
    pub context_digest: String,
    pub result_count: usize,
    pub page_trials: u64,
    pub job_trials: u64,
    pub continue_until_complete: bool,
    pub allow_cpu_fallback: bool,
    pub resume_token: Option<String>,
    pub cache_id: Option<String>,
}

/// The single-owner job store.
pub struct JobStore {
    inner: Mutex<Inner>,
    cancel: AtomicBool,
    alive: AtomicBool,
    secret: [u8; 32],
    context_digest: String,
    /// Compiles one query into the bounded collector for that job.
    factory: Option<Arc<dyn SearchFactory>>,
    source: Arc<dyn CandidateSource>,
    thread: Mutex<Option<JoinHandle<()>>>,
}

impl JobStore {
    /// Build a store. The session secret comes from the OS CSPRNG; a worker
    /// that cannot obtain one refuses to start rather than minting weak tokens.
    pub fn new(
        context_digest: &str,
        factory: Option<Arc<dyn SearchFactory>>,
        source: Arc<dyn CandidateSource>,
    ) -> Option<Self> {
        let mut secret = [0u8; 32];
        if !entropy::fill(&mut secret) {
            return None;
        }
        Some(Self {
            inner: Mutex::new(Inner {
                job: None,
                candidate_records: HashMap::new(),
                level: 180,
                maps: BTreeMap::new(),
            }),
            cancel: AtomicBool::new(false),
            alive: AtomicBool::new(false),
            secret,
            context_digest: context_digest.to_string(),
            factory,
            source,
            thread: Mutex::new(None),
        })
    }

    /// Whether a job thread is currently running.
    pub fn is_alive(&self) -> bool {
        self.alive.load(Ordering::SeqCst)
    }

    /// `SearchJobs.register_cache`.
    ///
    /// Validates one save-bound measured map against this worker's generation
    /// context and returns its content id. The id is the sha256 of the exact
    /// `cache_json` string, so two spellings of the same map are two ids, just
    /// like the shipped worker.
    pub fn register_cache(&self, cache_json: &str) -> Result<Value, RequestError> {
        let payload: Value = serde_json::from_str(cache_json)
            .map_err(|error| RequestError::invalid_request_message(error.to_string()))?;
        let mapping = grace_map::from_cache_payload(&payload, Some(&self.context_digest))
            .map_err(RequestError::invalid_request_message)?;
        let cache_id = hex_lower(&Sha256::digest(cache_json.as_bytes()));
        let mut inner = self.inner.lock().expect("job store");
        if !inner.maps.contains_key(&cache_id) && inner.maps.len() >= MAX_REGISTERED_MAPS {
            return Err(RequestError::invalid_request_message(
                "Measured map registry is full; restart the idle search worker",
            ));
        }
        inner.maps.insert(cache_id.clone(), mapping);
        Ok(serde_json::json!({ "cache_id": cache_id }))
    }

    /// `search.feasibility`: the structural preflight `start` applies, alone.
    ///
    /// Read-only and independent of the running job: it neither claims the
    /// worker nor touches any job. A query this preflight cannot judge (the
    /// save-bound NG4/NG5 route, or tables that failed to load) answers
    /// `checked: false` rather than a guess.
    pub fn feasibility(&self, query_payload: &Value) -> Result<Value, RequestError> {
        let mut query = SearchQuery::from_payload(query_payload)?;
        let unchecked = serde_json::json!({ "checked": false, "feasible": true, "reason": null });
        if query.playthrough > 3 || self.source.resolve_query(&mut query).is_err() {
            return Ok(unchecked);
        }
        let Some(factory) = &self.factory else {
            return Ok(unchecked);
        };
        Ok(match factory.feasibility(&query) {
            None => unchecked,
            Some(Ok(())) => {
                serde_json::json!({ "checked": true, "feasible": true, "reason": null })
            }
            Some(Err(reason)) => {
                serde_json::json!({ "checked": true, "feasible": false, "reason": reason })
            }
        })
    }

    /// `SearchJobs.start`.
    pub fn start(
        self: &Arc<Self>,
        query_payload: &Value,
        params: &StartParams,
    ) -> Result<JobView, RequestError> {
        // Claim the single owner atomically. A plain `is_alive()` load followed by
        // a later store loses the race: two concurrent starts can both observe an
        // idle worker, and the second then replaces the first job's record and
        // leaves its thread unowned. The compare-exchange makes the check and the
        // claim one step, so exactly one caller can own the worker at a time even
        // though the framed main loop is single-threaded and only a caller that
        // issues starts in parallel can reach this path.
        if self
            .alive
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err(RequestError::new("BUSY", "One search can run per worker"));
        }
        // The previous owner is finished (`is_alive` is false), so joining it
        // here cannot block; it keeps one job thread per store.
        if let Some(previous) = self.thread.lock().expect("job thread").take() {
            let _ = previous.join();
        }
        // From here the claim must be released on every early return.
        if params.context_digest != self.context_digest {
            self.alive.store(false, Ordering::Release);
            return Err(RequestError::new(
                "CONTEXT_MISMATCH",
                "Refresh the worker handshake before searching",
            ));
        }
        let mut query = match SearchQuery::from_payload(query_payload) {
            Ok(query) => query,
            Err(error) => {
                self.alive.store(false, Ordering::Release);
                return Err(error);
            }
        };
        // Terrain options and Grace choices are resolved against the same
        // context-bound tables the candidates are composed from, once, so the
        // compiler, the page filters and the final acceptance agree.
        if let Err(error) = self.source.resolve_query(&mut query) {
            self.alive.store(false, Ordering::Release);
            return Err(match error.code.as_str() {
                "INVALID_REQUEST" => RequestError::invalid_request_message(error.message),
                _ => RequestError::new("RESOURCE_MISMATCH", error.message),
            });
        }
        // `SearchJobs.start`'s cache binding. NG4/NG5 runs only against an exact
        // save-bound rarity-5 map whose record type matches the playthrough;
        // NG3 always uses its certified bundled map and never takes a cache.
        let mapping = {
            let inner = self.inner.lock().expect("job store");
            params
                .cache_id
                .as_ref()
                .and_then(|cache_id| inner.maps.get(cache_id).cloned())
        };
        if query.playthrough > 3 {
            let expected_record_type = CATEGORY_TO_TYPE
                .get(usize::from(query.playthrough))
                .copied();
            let usable = query.rarity == 5
                && mapping.as_ref().is_some_and(|mapping| {
                    Some(mapping.record_type) == expected_record_type && mapping.rarity == 5
                });
            if !usable {
                self.alive.store(false, Ordering::Release);
                return Err(RequestError::invalid_request_message(
                    "NG4/5 offline search requires an exact save-bound rarity-5 map",
                ));
            }
        } else if params.cache_id.is_some() {
            self.alive.store(false, Ordering::Release);
            return Err(RequestError::invalid_request_message(
                "NG3 uses its certified bundled map",
            ));
        }
        let binding = binding(
            &query.digest,
            &params.context_digest,
            params.allow_cpu_fallback,
            params.continue_until_complete,
            params.cache_id.as_deref(),
        );
        let cursor = match self.decode_cursor(params.resume_token.as_deref(), &binding) {
            Ok(cursor) => cursor,
            Err(error) => {
                self.alive.store(false, Ordering::Release);
                return Err(error);
            }
        };
        // The table-derived compilation runs at start, so an unsupported filter
        // fails the request with a message naming it instead of starting a job
        // that can only fail.
        let collector = match &self.factory {
            Some(factory) => {
                let compiled = match (&mapping, query.playthrough) {
                    (Some(cache), 4 | 5) => factory.cached_collector(&query, cache),
                    _ => factory.collector(&query),
                }
                .map_err(|error| match error.code.as_str() {
                    // A backend that cannot run at all keeps its own code so a
                    // caller can tell "no accelerator" apart from a bad query.
                    "SEARCH_BACKEND_UNAVAILABLE" => {
                        RequestError::new("SEARCH_BACKEND_UNAVAILABLE", error.message)
                    }
                    "SEARCH_FAILED" => RequestError::new("SEARCH_FAILED", error.message),
                    // Compilation rejections name the unsupported filter.
                    _ => RequestError::invalid_request_message(error.message),
                });
                match compiled {
                    Ok(collector) => Some(collector),
                    Err(error) => {
                        self.alive.store(false, Ordering::Release);
                        return Err(error);
                    }
                }
            }
            None => None,
        };

        self.cancel.store(false, Ordering::SeqCst);
        let job = {
            let mut inner = self.inner.lock().expect("job store");
            inner.candidate_records.clear();
            inner.level = query.level;
            inner.job = Some(JobState {
                job_id: entropy::uuid4(),
                state: "queued",
                sequence: 0,
                query_digest: query.digest.clone(),
                context_digest: params.context_digest.clone(),
                cursor,
                start_cursor: cursor,
                candidates: Vec::new(),
                progress: None,
                stop_reason: None,
                error: None,
                resume_token: None,
                elapsed_ms: 0,
            });
            inner.job.as_ref().expect("job just stored").view()
        };

        // `alive` is already claimed by the compare-exchange above.
        let store = Arc::clone(self);
        let params = params.clone();
        let handle = thread::Builder::new()
            .name("offline-search".to_string())
            .spawn(move || store.run(query, params, binding, collector))
            .expect("spawn the offline search thread");
        *self.thread.lock().expect("job thread") = Some(handle);
        Ok(job)
    }

    /// `SearchJobs.snapshot`.
    pub fn snapshot(&self, job_id: &str) -> Result<JobView, RequestError> {
        let inner = self.inner.lock().expect("job store");
        match &inner.job {
            Some(job) if job.job_id == job_id => Ok(job.view()),
            _ => Err(RequestError::job_not_found()),
        }
    }

    /// `SearchJobs.current`.
    pub fn current(&self) -> Option<JobView> {
        let inner = self.inner.lock().expect("job store");
        inner.job.as_ref().map(JobState::view)
    }

    /// `SearchJobs.export`.
    pub fn export(&self, job_id: &str, candidate_id: &str) -> Result<Value, RequestError> {
        let inner = self.inner.lock().expect("job store");
        match &inner.job {
            Some(job) if job.job_id == job_id => {}
            _ => return Err(RequestError::job_not_found()),
        }
        let Some(candidate) = inner.candidate_records.get(candidate_id) else {
            return Err(RequestError::invalid_request_message(
                "Candidate is no longer retained by this search job",
            ));
        };
        Ok(payload::transfer_json(
            candidate,
            &self.context_digest,
            inner.level,
        ))
    }

    /// `SearchJobs.cancel`.
    pub fn cancel(&self, job_id: &str) -> Result<JobView, RequestError> {
        let mut inner = self.inner.lock().expect("job store");
        let found = inner.job.as_ref().is_some_and(|job| job.job_id == job_id);
        if !found {
            return Err(RequestError::job_not_found());
        }
        let job = inner.job.as_mut().expect("checked above");
        if !TERMINAL_STATES.contains(&job.state) {
            self.cancel.store(true, Ordering::SeqCst);
            job.state = "cancel_requested";
            job.sequence += 1;
        }
        Ok(job.view())
    }

    /// `SearchJobs.shutdown`: cancel, then join the job thread.
    pub fn shutdown(&self) {
        {
            let inner = self.inner.lock().expect("job store");
            let cancel_now = inner
                .job
                .as_ref()
                .is_some_and(|job| !TERMINAL_STATES.contains(&job.state));
            if cancel_now {
                self.cancel.store(true, Ordering::SeqCst);
            }
        }
        if let Some(handle) = self.thread.lock().expect("job thread").take() {
            let _ = handle.join();
        }
    }

    fn run(
        self: Arc<Self>,
        query: SearchQuery,
        params: StartParams,
        binding: String,
        collector: Option<Arc<dyn SearchCollector>>,
    ) {
        let started = Instant::now();
        let outcome = self.search(&query, &params, started, collector.as_ref());
        {
            let mut inner = self.inner.lock().expect("job store");
            if let Some(job) = inner.job.as_mut() {
                match &outcome {
                    Ok((reason, cursor)) => {
                        let cancelled = self.cancel.load(Ordering::SeqCst);
                        let reason: &'static str = if cancelled { "cancelled" } else { reason };
                        job.state = if reason == "cancelled" {
                            "cancelled"
                        } else {
                            "completed"
                        };
                        job.stop_reason = Some(reason);
                        job.resume_token = if reason == "family_exhausted" {
                            None
                        } else {
                            Some(self.mint_token(&binding, *cursor))
                        };
                        job.cursor = *cursor;
                    }
                    Err(error) => {
                        job.state = "failed";
                        job.stop_reason = Some("error");
                        job.resume_token = None;
                        job.error = Some((error.code.clone(), error.message.clone()));
                    }
                }
                job.sequence += 1;
                job.elapsed_ms = started.elapsed().as_millis() as u64;
            }
        }
        self.alive.store(false, Ordering::SeqCst);
    }

    /// The bounded page loop, mirroring `SearchJobs._run`.
    fn search(
        &self,
        query: &SearchQuery,
        params: &StartParams,
        started: Instant,
        job_collector: Option<&Arc<dyn SearchCollector>>,
    ) -> Result<(&'static str, u64), CollectorError> {
        // The registered save-bound map of this job, when it has one. It is the
        // same map the collector compiled against, so materialization composes
        // the player's own playthrough instead of the bundled NG3 tables.
        let cached_grace: Option<GraceOutputMap> = params.cache_id.as_ref().and_then(|cache_id| {
            self.inner
                .lock()
                .expect("job store")
                .maps
                .get(cache_id)
                .cloned()
        });
        let cached_grace = cached_grace.and_then(|mapping| mapping.to_domain_map().ok());
        let mut cursor = self
            .inner
            .lock()
            .expect("job store")
            .job
            .as_ref()
            .map(|job| job.cursor)
            .unwrap_or(0);
        {
            let mut inner = self.inner.lock().expect("job store");
            let cancelled = self.cancel.load(Ordering::SeqCst);
            if let Some(job) = inner.job.as_mut() {
                if !cancelled {
                    job.state = "running";
                    job.sequence += 1;
                }
            }
        }

        let stop = if params.continue_until_complete {
            None
        } else {
            Some(cursor + params.job_trials)
        };
        let mut reason: &'static str = "budget_reached";
        let collector =
            job_collector.ok_or_else(|| CollectorError::unavailable(MISSING_COLLECTOR_MESSAGE))?;

        while stop.is_none_or(|stop| cursor < stop) {
            if self.cancel.load(Ordering::SeqCst) {
                reason = "cancelled";
                break;
            }
            // One native unit per page, so each unit's matches reach the job
            // (and the interface) as soon as it finishes.
            let requested = collector
                .native_unit_trials()
                .map_or(params.page_trials, |unit| params.page_trials.min(unit));
            let page_budget = match stop {
                None => requested,
                Some(stop) => requested.min(stop - cursor),
            };
            let remaining = params.result_count.saturating_sub(self.committed());
            let mut progress = |report: &IntersectionReport| {
                let mut inner = self.inner.lock().expect("job store");
                if let Some(job) = inner.job.as_mut() {
                    job.progress = Some(report.clone());
                    job.sequence += 1;
                    job.elapsed_ms = started.elapsed().as_millis() as u64;
                }
            };
            let cancelled = || self.cancel.load(Ordering::SeqCst);
            let batch = collector.collect(
                &BatchRequest {
                    query,
                    level: query.level,
                    result_count: remaining,
                    max_trials_per_batch: page_budget,
                    start_after_trial: cursor,
                    allow_cpu_fallback: params.allow_cpu_fallback,
                },
                &mut progress,
                &cancelled,
            )?;
            let remaining = params.result_count.saturating_sub(self.committed());
            if batch.matches.len() > remaining {
                return Err(CollectorError::new(
                    "RESULT_OVERFLOW",
                    "Solver exceeded the requested result limit",
                ));
            }

            // Materialize only the matches of this bounded page, then apply the
            // job-level filters the solver prefilters cannot express.
            let mut payloads = Vec::new();
            let mut accepted = Vec::new();
            for pivot in &batch.matches {
                let materialized = self
                    .source
                    .materialize(query, pivot.seed, pivot.trial, cached_grace.as_ref())
                    .map_err(|error| {
                        // Name the exact match that could not be composed so a
                        // bounded port limit is reproducible from the job error
                        // instead of being rediscovered by bisection.
                        CollectorError::new(
                            error.code,
                            format!(
                                "{} (seed {}, trial {}, rarity {}, level {})",
                                error.message, pivot.seed, pivot.trial, query.rarity, query.level
                            ),
                        )
                    })?;
                if candidate_ready(&materialized.candidate) && accepts(query, &materialized) {
                    accepted.push(materialized.candidate);
                    payloads.push(materialized.payload);
                }
            }

            let Some(next_cursor) = batch.next_start_after_trial else {
                return Err(CollectorError::new(
                    "INVALID_CHECKPOINT",
                    "Solver returned an invalid page cursor",
                ));
            };
            let page_limit = cursor + page_budget;
            if next_cursor < cursor || next_cursor > page_limit {
                return Err(CollectorError::new(
                    "INVALID_CHECKPOINT",
                    "Solver returned an invalid page cursor",
                ));
            }
            {
                let mut inner = self.inner.lock().expect("job store");
                let committed = inner
                    .job
                    .as_ref()
                    .map(|job| job.candidates.len())
                    .unwrap_or(0);
                if committed + payloads.len() > params.result_count {
                    return Err(CollectorError::new(
                        "RESULT_OVERFLOW",
                        "Solver exceeded the requested result limit",
                    ));
                }
                let context_digest = self.context_digest.clone();
                let records: Vec<(String, Candidate)> = accepted
                    .iter()
                    .map(|candidate| {
                        (
                            crate::model::candidate_identity(candidate, &context_digest),
                            candidate.clone(),
                        )
                    })
                    .collect();
                let job = inner.job.as_mut().expect("job exists while running");
                job.candidates.extend(payloads);
                job.cursor = next_cursor;
                job.sequence += 1;
                for (candidate_id, candidate) in records {
                    inner.candidate_records.insert(candidate_id, candidate);
                }
            }

            let previous = cursor;
            cursor = next_cursor;
            if self.cancel.load(Ordering::SeqCst) {
                reason = "cancelled";
                break;
            }
            if self.committed() >= params.result_count {
                reason = "result_limit";
                break;
            }
            if batch
                .intersection_report
                .as_ref()
                .is_some_and(|report| report.exhausted_family)
            {
                reason = "family_exhausted";
                break;
            }
            if cursor == previous {
                return Err(CollectorError::new(
                    "NO_PROGRESS",
                    "Solver stopped without an exhaustion checkpoint",
                ));
            }
        }
        Ok((reason, cursor))
    }

    fn committed(&self) -> usize {
        self.inner
            .lock()
            .expect("job store")
            .job
            .as_ref()
            .map(|job| job.candidates.len())
            .unwrap_or(0)
    }

    fn mint_token(&self, binding: &str, cursor: u64) -> String {
        let body = format!("{{\"binding\": \"{binding}\", \"cursor\": {cursor}}}");
        let signature = hmac_sha256(&self.secret, body.as_bytes());
        format!(
            "{}.{}",
            base64url_encode(body.as_bytes()),
            hex_lower(&signature)
        )
    }

    fn decode_cursor(&self, token: Option<&str>, binding: &str) -> Result<u64, RequestError> {
        let Some(token) = token else {
            return Ok(0);
        };
        let invalid = RequestError::invalid_resume_token;
        let (encoded, signature) = token.split_once('.').ok_or_else(invalid)?;
        let body = base64url_decode(encoded).ok_or_else(invalid)?;
        let expected = hex_lower(&hmac_sha256(&self.secret, &body));
        if !constant_time_eq(signature.as_bytes(), expected.as_bytes()) {
            return Err(invalid());
        }
        let parsed: Value = serde_json::from_slice(&body).map_err(|_| invalid())?;
        if parsed.get("binding").and_then(Value::as_str) != Some(binding) {
            return Err(invalid());
        }
        parsed
            .get("cursor")
            .and_then(Value::as_u64)
            .ok_or_else(invalid)
    }
}

/// The resume-token binding, exactly `f'{digest}:{ctx}:{allow}:{policy}:{cache}'`
/// with Python's `True`/`False`/`None` spellings.
pub fn binding(
    query_digest: &str,
    context_digest: &str,
    allow_cpu_fallback: bool,
    continue_until_complete: bool,
    cache_id: Option<&str>,
) -> String {
    format!(
        "{query_digest}:{context_digest}:{}:{}:{}",
        python_bool(allow_cpu_fallback),
        python_bool(continue_until_complete),
        cache_id.unwrap_or("None"),
    )
}

fn python_bool(value: bool) -> &'static str {
    if value {
        "True"
    } else {
        "False"
    }
}

/// `search_application.require_search_candidate_ready`: rarity-4 stage-one
/// records never join a result list.
fn candidate_ready(candidate: &Candidate) -> bool {
    !(candidate.rarity == 4 && candidate.record_stage == crate::model::RecordStage::NativeStageOne)
}

/// The job-level filters `SearchJobs._run` applies after the solver prefilters.
/// The post-acceptance filters `SearchJobs._run` applies after the solver prefilters:
/// `enemy_occurrence_groups`, `initial_challenge_counts`, `grace_effect_ids` and
/// the grouped-roll / occurrence constraints.
///
/// These are enforced here and never assumed to be native-filtered, because the
/// compiled pivot deliberately does not narrow on them.
fn accepts(query: &SearchQuery, materialized: &MaterializedCandidate) -> bool {
    if materialized.auxiliary_match != Some(true) {
        return false;
    }
    if materialized.enemy_occurrence_match != Some(true) {
        return false;
    }
    let candidate = &materialized.candidate;
    if !query.initial_challenge_counts.is_empty()
        && !query.initial_challenge_counts.contains(
            &(nioh3_domain::sequence::generate_challenge_attempt_count(candidate.seed) as u8),
        )
    {
        return false;
    }
    // `c.grace is not None and c.grace.effect_id in query.grace_effect_ids`.
    // The UI always sends a single choice in the list as well; a lone
    // `grace_effect_id` is held to the same final-Grace test so no route can
    // publish a candidate whose actual Grace differs from the selection.
    let graces: Vec<u32> = if query.grace_effect_ids.is_empty() {
        query.grace_effect_id.into_iter().collect()
    } else {
        query.grace_effect_ids.clone()
    };
    if !graces.is_empty()
        && !candidate_grace(candidate).is_some_and(|effect_id| graces.contains(&effect_id))
    {
        return false;
    }
    if !query.grouped_rolls.is_empty() {
        // `c.effects[0 if not query.request.primary_effect_ids else 1:]`.
        let offset = usize::from(!query.primary_effect_ids.is_empty());
        let effects = candidate.effects.get(offset..).unwrap_or(&[]);
        for group in &query.required_secondary_id_groups {
            let satisfied = group.iter().any(|effect_id| {
                effects.iter().any(|effect| {
                    effect.effect_id == *effect_id
                        && effect.roll_percent.is_some_and(|roll| {
                            let minimum = query
                                .grouped_rolls
                                .iter()
                                .find(|(id, _)| id == effect_id)
                                .map(|(_, minimum)| *minimum)
                                .unwrap_or(0);
                            roll >= minimum
                        })
                })
            });
            if !satisfied {
                return false;
            }
        }
    }
    if !query.effect_occurrences.is_empty() && !matches_occurrences(candidate, query) {
        return false;
    }
    true
}

/// `models.ScrollCandidate.grace`: the final Grace a composed candidate carries.
///
/// Rarity 5 always ends in its Grace (zero-based index 5). A rarity-4 record
/// keeps a Grace only when its fifth effect survived finalization as a
/// verified final Grace id with the fixed bit set; a slot the finalizer
/// replaced with an ordinary effect is not a Grace even if the id coincides.
pub(crate) fn candidate_grace(candidate: &Candidate) -> Option<u32> {
    match candidate.rarity {
        5 => candidate.effects.get(5).map(|effect| effect.effect_id),
        4 => candidate.effects.get(4).and_then(|effect| {
            (crate::catalog::R4_FINAL_GRACE_IDS.contains(&effect.effect_id)
                && (effect.metadata >> 16) & 0x02 != 0)
                .then_some(effect.effect_id)
        }),
        _ => None,
    }
}

/// `effect_occurrences.matches_occurrences`: display-slot requirements without
/// reusing one effect occurrence.
fn matches_occurrences(candidate: &Candidate, query: &SearchQuery) -> bool {
    let mut choices: Vec<Vec<usize>> = Vec::with_capacity(query.effect_occurrences.len());
    for requirement in &query.effect_occurrences {
        let mut eligible = Vec::new();
        for (slot, effect) in candidate.effects.iter().enumerate() {
            match requirement.scope {
                OccurrenceScope::Primary if slot != 0 => continue,
                OccurrenceScope::Secondary if slot == 0 => continue,
                _ => {}
            }
            if requirement.alternatives.iter().any(|alternative| {
                alternative.effect_id == effect.effect_id
                    && (alternative.minimum_roll_percent == 0
                        || effect
                            .roll_percent
                            .is_some_and(|roll| roll >= alternative.minimum_roll_percent))
            }) {
                eligible.push(slot);
            }
        }
        if eligible.is_empty() {
            return false;
        }
        choices.push(eligible);
    }
    choices.sort_by_key(Vec::len);
    assign_occurrences(&choices, 0, 0)
}

fn assign_occurrences(choices: &[Vec<usize>], index: usize, used: u64) -> bool {
    if index == choices.len() {
        return true;
    }
    choices[index]
        .iter()
        .filter(|slot| used & (1u64 << **slot) == 0)
        .any(|slot| assign_occurrences(choices, index + 1, used | (1u64 << *slot)))
}

fn hex_lower(bytes: &[u8]) -> String {
    crate::context::hex_lower(bytes)
}

/// RFC 2104 HMAC-SHA256 over the pinned `sha2` digest.
pub fn hmac_sha256(key: &[u8], message: &[u8]) -> [u8; 32] {
    const BLOCK: usize = 64;
    let mut key_block = [0u8; BLOCK];
    if key.len() > BLOCK {
        let digest = Sha256::digest(key);
        key_block[..32].copy_from_slice(&digest);
    } else {
        key_block[..key.len()].copy_from_slice(key);
    }
    let mut inner_pad = [0x36u8; BLOCK];
    let mut outer_pad = [0x5cu8; BLOCK];
    for index in 0..BLOCK {
        inner_pad[index] ^= key_block[index];
        outer_pad[index] ^= key_block[index];
    }
    let mut inner = Sha256::new();
    inner.update(inner_pad);
    inner.update(message);
    let inner = inner.finalize();
    let mut outer = Sha256::new();
    outer.update(outer_pad);
    outer.update(inner);
    outer.finalize().into()
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut difference = 0u8;
    for (left, right) in left.iter().zip(right) {
        difference |= left ^ right;
    }
    difference == 0
}

const BASE64URL: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/// `base64.urlsafe_b64encode`, including padding.
fn base64url_encode(data: &[u8]) -> String {
    let mut output = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let triple = ((chunk[0] as u32) << 16)
            | ((chunk.get(1).copied().unwrap_or(0) as u32) << 8)
            | (chunk.get(2).copied().unwrap_or(0) as u32);
        output.push(BASE64URL[(triple >> 18) as usize & 0x3F] as char);
        output.push(BASE64URL[(triple >> 12) as usize & 0x3F] as char);
        output.push(if chunk.len() > 1 {
            BASE64URL[(triple >> 6) as usize & 0x3F] as char
        } else {
            '='
        });
        output.push(if chunk.len() > 2 {
            BASE64URL[triple as usize & 0x3F] as char
        } else {
            '='
        });
    }
    output
}

fn base64url_decode(text: &str) -> Option<Vec<u8>> {
    let bytes = text.as_bytes();
    if !bytes.len().is_multiple_of(4) {
        return None;
    }
    let mut output = Vec::with_capacity(bytes.len() / 4 * 3);
    for chunk in bytes.chunks(4) {
        let mut values = [0u32; 4];
        let mut padding = 0;
        for (index, byte) in chunk.iter().enumerate() {
            if *byte == b'=' {
                values[index] = 0;
                padding += 1;
            } else {
                values[index] = BASE64URL.iter().position(|candidate| candidate == byte)? as u32;
            }
        }
        if padding > 2 {
            return None;
        }
        let triple = (values[0] << 18) | (values[1] << 12) | (values[2] << 6) | values[3];
        output.push((triple >> 16) as u8);
        if padding < 2 {
            output.push((triple >> 8) as u8);
        }
        if padding < 1 {
            output.push(triple as u8);
        }
    }
    Some(output)
}

/// Session randomness. The worker refuses to start without a real CSPRNG.
mod entropy {
    #[cfg(windows)]
    mod platform {
        use std::ffi::c_void;

        #[link(name = "bcrypt")]
        extern "system" {
            fn BCryptGenRandom(
                algorithm: *mut c_void,
                buffer: *mut u8,
                length: u32,
                flags: u32,
            ) -> i32;
        }

        /// `BCRYPT_USE_SYSTEM_PREFERRED_RNG`.
        const USE_SYSTEM_PREFERRED_RNG: u32 = 0x0000_0002;

        pub fn fill(buffer: &mut [u8]) -> bool {
            if buffer.is_empty() {
                return true;
            }
            let status = unsafe {
                // SAFETY: a null algorithm handle with
                // BCRYPT_USE_SYSTEM_PREFERRED_RNG is the documented system RNG
                // call; the buffer and length come straight from the caller.
                BCryptGenRandom(
                    std::ptr::null_mut(),
                    buffer.as_mut_ptr(),
                    buffer.len() as u32,
                    USE_SYSTEM_PREFERRED_RNG,
                )
            };
            status == 0
        }
    }

    #[cfg(not(windows))]
    mod platform {
        use std::io::Read;

        pub fn fill(buffer: &mut [u8]) -> bool {
            match std::fs::File::open("/dev/urandom") {
                Ok(mut file) => file.read_exact(buffer).is_ok(),
                Err(_) => false,
            }
        }
    }

    pub fn fill(buffer: &mut [u8]) -> bool {
        platform::fill(buffer)
    }

    /// UUID v4, formatted like `str(uuid.uuid4())`.
    pub fn uuid4() -> String {
        let mut bytes = [0u8; 16];
        if !fill(&mut bytes) {
            panic!("the operating system CSPRNG is unavailable");
        }
        bytes[6] = (bytes[6] & 0x0F) | 0x40;
        bytes[8] = (bytes[8] & 0x3F) | 0x80;
        let hex = super::hex_lower(&bytes);
        format!(
            "{}-{}-{}-{}-{}",
            &hex[0..8],
            &hex[8..12],
            &hex[12..16],
            &hex[16..20],
            &hex[20..32]
        )
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::{Condvar, Mutex as StdMutex};
    use std::time::Duration;

    use serde_json::json;

    use super::*;
    use crate::collector::{IntersectionStageCount, SearchBatch};
    use crate::model::{CandidateEffect, RecordStage};
    use crate::native_search::PivotMatch;

    const CONTEXT: &str = "3a4cf53e9e77729d3a30a13199cbafffe2bfcabcd96540fad3a7ab048be5d6ae";

    fn report(exhausted: bool, start: u64, through: u64, family: u64) -> IntersectionReport {
        IntersectionReport {
            start_after_trial: start,
            inspected_through_trial: through,
            family_size: family,
            fixed_seed_count: through.saturating_sub(start),
            stages: vec![IntersectionStageCount {
                kind: "special_rule".to_string(),
                values: vec![64956, 113, 20893],
                count: 1,
            }],
            complete_match_count: 1,
            exhausted_family: exhausted,
        }
    }

    fn page(next: u64, matches: Vec<PivotMatch>, exhausted: bool) -> SearchBatch {
        SearchBatch {
            matches,
            next_start_after_trial: Some(next),
            intersection_report: Some(report(exhausted, 0, next, 1 << 32)),
            streamed: true,
        }
    }

    /// Deterministic per-page source.
    struct ScriptedCollector {
        pages: StdMutex<Vec<Result<SearchBatch, CollectorError>>>,
        calls: AtomicU64,
        seen: StdMutex<Vec<(u64, u64, usize)>>,
        policies: StdMutex<Vec<bool>>,
    }

    impl ScriptedCollector {
        fn new(pages: Vec<Result<SearchBatch, CollectorError>>) -> Arc<Self> {
            Arc::new(Self {
                pages: StdMutex::new(pages),
                calls: AtomicU64::new(0),
                seen: StdMutex::new(Vec::new()),
                policies: StdMutex::new(Vec::new()),
            })
        }

        fn seen(&self) -> Vec<(u64, u64, usize)> {
            self.seen.lock().expect("seen").clone()
        }

        /// The `allow_cpu_fallback` flag of every page, in call order.
        fn policies(&self) -> Vec<bool> {
            self.policies.lock().expect("policies").clone()
        }
    }

    impl SearchCollector for ScriptedCollector {
        fn collect(
            &self,
            request: &BatchRequest<'_>,
            progress: &mut dyn FnMut(&IntersectionReport),
            _cancelled: &dyn Fn() -> bool,
        ) -> Result<SearchBatch, CollectorError> {
            let index = self.calls.fetch_add(1, Ordering::SeqCst);
            self.seen.lock().expect("seen").push((
                request.start_after_trial,
                request.max_trials_per_batch,
                request.result_count,
            ));
            self.policies
                .lock()
                .expect("policies")
                .push(request.allow_cpu_fallback);
            let page = self
                .pages
                .lock()
                .expect("pages")
                .get(index as usize)
                .cloned()
                .unwrap_or_else(|| Ok(SearchBatch::default()));
            if let Ok(batch) = &page {
                if let Some(report) = &batch.intersection_report {
                    progress(report);
                }
            }
            page
        }
    }

    /// Blocks inside the page until released, then honours the cancel poll.
    struct GatedCollector {
        gate: StdMutex<bool>,
        signal: Condvar,
    }

    impl GatedCollector {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                gate: StdMutex::new(false),
                signal: Condvar::new(),
            })
        }

        fn release(&self) {
            let mut gate = self.gate.lock().expect("gate");
            *gate = true;
            self.signal.notify_all();
        }
    }

    impl SearchCollector for GatedCollector {
        fn collect(
            &self,
            request: &BatchRequest<'_>,
            _progress: &mut dyn FnMut(&IntersectionReport),
            cancelled: &dyn Fn() -> bool,
        ) -> Result<SearchBatch, CollectorError> {
            let mut gate = self.gate.lock().expect("gate");
            while !*gate {
                gate = self
                    .signal
                    .wait_timeout(gate, Duration::from_secs(30))
                    .expect("gate wait")
                    .0;
            }
            let next = request.start_after_trial + 16;
            if cancelled() {
                return Ok(SearchBatch {
                    matches: Vec::new(),
                    next_start_after_trial: Some(next),
                    intersection_report: Some(report(
                        false,
                        request.start_after_trial,
                        next,
                        1 << 20,
                    )),
                    streamed: false,
                });
            }
            Ok(SearchBatch {
                matches: vec![PivotMatch {
                    seed: 42,
                    trial: next,
                }],
                next_start_after_trial: Some(next),
                intersection_report: Some(report(true, request.start_after_trial, next, 1 << 20)),
                streamed: true,
            })
        }
    }

    /// Mirrors the shipped `materialize_match` without needing data files.
    struct FakeSource {
        enemy_occurrence_match: Option<bool>,
        auxiliary_match: Option<bool>,
    }

    impl CandidateSource for FakeSource {
        fn materialize(
            &self,
            _query: &SearchQuery,
            seed: u32,
            trial: u64,
            _grace: Option<&nioh3_domain::effect::GraceMap>,
        ) -> Result<MaterializedCandidate, CollectorError> {
            let rarity = 4;
            let candidate = Candidate {
                seed,
                playthrough: Some(3),
                rarity,
                record_stage: RecordStage::EffectSequenceOnly,
                record: Vec::new(),
                installation_record: None,
                effects: vec![CandidateEffect {
                    slot: 1,
                    effect_id: 64956,
                    value: 1,
                    metadata: 0,
                    prefix: 0,
                    tail_0: 0,
                    tail_1: 0,
                    roll_percent: Some(80),
                }],
                joint_search_trial: Some(trial),
            };
            let candidate_id = crate::model::candidate_identity(&candidate, CONTEXT);
            Ok(MaterializedCandidate {
                payload: json!({
                    "candidate_id": candidate_id,
                    "context_digest": CONTEXT,
                    "seed": seed,
                    "playthrough": 3,
                    "rarity": rarity,
                    "record_stage": "effect_sequence_only",
                    "installable": true,
                    "install_blocker": null,
                    "effects": [],
                    "auxiliary": null,
                    "enemy_states": null,
                    "cursor": trial,
                    "evidence": "certified_offline_replay",
                    "installation_available": true,
                    "initial_challenge_capacity": 4,
                }),
                candidate,
                auxiliary_match: self.auxiliary_match,
                enemy_occurrence_match: self.enemy_occurrence_match,
            })
        }
    }

    fn query_payload(rarity: u8, rule_shape: &str) -> Value {
        let auxiliary = match rule_shape {
            "groups" => json!({
                "required_terrain_effect_keys": [],
                "required_terrain_effect_key_groups": [],
                "required_special_rule_keys": [],
                "required_special_rule_key_groups": [[64956], [113], [20893]],
                "required_enemy_lookup_keys": [],
                "required_enemy_lookup_key_groups": []
            }),
            _ => json!({
                "required_terrain_effect_keys": [],
                "required_terrain_effect_key_groups": [],
                "required_special_rule_keys": [64956, 113, 20893],
                "required_special_rule_key_groups": [],
                "required_enemy_lookup_keys": [],
                "required_enemy_lookup_key_groups": []
            }),
        };
        json!({
            "playthrough": 3,
            "rarity": rarity,
            "level": 180,
            "primary_effect_ids": [],
            "required_secondary_ids": [],
            "required_secondary_id_groups": [],
            "grace_effect_id": null,
            "minimum_roll_percent_by_effect_id": [],
            "auxiliary": auxiliary
        })
    }

    fn params(continue_until_complete: bool) -> StartParams {
        StartParams {
            context_digest: CONTEXT.to_string(),
            result_count: 1,
            page_trials: 100_000_000,
            job_trials: 10_000_000,
            continue_until_complete,
            allow_cpu_fallback: false,
            resume_token: None,
            cache_id: None,
        }
    }

    /// Hands the same scripted collector to every job.
    struct FixedFactory(Arc<dyn SearchCollector>);

    impl SearchFactory for FixedFactory {
        fn collector(
            &self,
            _query: &SearchQuery,
        ) -> Result<Arc<dyn SearchCollector>, CollectorError> {
            Ok(Arc::clone(&self.0))
        }
    }

    /// Rejects every query with a named filter, like an unsupported route.
    struct RejectingFactory(&'static str);

    impl SearchFactory for RejectingFactory {
        fn collector(
            &self,
            _query: &SearchQuery,
        ) -> Result<Arc<dyn SearchCollector>, CollectorError> {
            Err(CollectorError::new(
                "UNSUPPORTED_QUERY",
                format!("this development worker cannot pack the {} filter", self.0),
            ))
        }
    }

    /// Judges every query infeasible, like the native structural preflight.
    struct InfeasibleFactory;

    impl SearchFactory for InfeasibleFactory {
        fn feasibility(&self, _query: &SearchQuery) -> Option<Result<(), String>> {
            Some(Err("category overflow".to_string()))
        }

        fn collector(
            &self,
            _query: &SearchQuery,
        ) -> Result<Arc<dyn SearchCollector>, CollectorError> {
            Err(CollectorError::new("INVALID_REQUEST", "category overflow"))
        }
    }

    #[test]
    fn feasibility_answers_without_claiming_the_worker() {
        let store = open_store_with(Some(Arc::new(InfeasibleFactory)));
        let refused = store
            .feasibility(&query_payload(4, "flat"))
            .expect("a schema-valid query is judged");
        assert_eq!(refused["checked"], true);
        assert_eq!(refused["feasible"], false);
        assert_eq!(refused["reason"], "category overflow");
        assert!(!store.alive.load(Ordering::Acquire), "no job was claimed");

        let unjudged = open_store_with(Some(Arc::new(RejectingFactory("any"))))
            .feasibility(&query_payload(4, "flat"))
            .expect("judged");
        assert_eq!(unjudged["checked"], false);
        assert_eq!(unjudged["feasible"], true);
    }

    fn open_store_with(factory: Option<Arc<dyn SearchFactory>>) -> Arc<JobStore> {
        Arc::new(
            JobStore::new(
                CONTEXT,
                factory,
                Arc::new(FakeSource {
                    enemy_occurrence_match: Some(true),
                    auxiliary_match: Some(true),
                }),
            )
            .expect("an OS CSPRNG is available"),
        )
    }

    fn open_store(collector: Arc<dyn SearchCollector>) -> Arc<JobStore> {
        open_store_with(Some(Arc::new(FixedFactory(collector))))
    }

    fn ramped_pages(count: u64, step: u64) -> Arc<ScriptedCollector> {
        ScriptedCollector::new(
            (0..count)
                .map(|index| {
                    Ok(SearchBatch {
                        matches: Vec::new(),
                        next_start_after_trial: Some((index + 1) * step),
                        intersection_report: Some(report(
                            false,
                            index * step,
                            (index + 1) * step,
                            1 << 32,
                        )),
                        streamed: false,
                    })
                })
                .collect(),
        )
    }

    fn wait_for(predicate: impl Fn() -> bool) {
        for _ in 0..3_000 {
            if predicate() {
                return;
            }
            thread::sleep(Duration::from_millis(2));
        }
        panic!("condition never became true");
    }

    fn wait_terminal(store: &JobStore, job_id: &str) -> JobView {
        wait_for(|| {
            store
                .snapshot(job_id)
                .is_ok_and(|job| TERMINAL_STATES.contains(&job.state))
        });
        store.snapshot(job_id).expect("job exists")
    }

    fn finished_job(store: &Arc<JobStore>, payload: &Value) -> JobView {
        let started = store.start(payload, &params(true)).expect("start accepted");
        wait_terminal(store, &started.job_id)
    }

    #[test]
    fn continuation_runs_pages_until_the_result_limit() {
        let collector = ScriptedCollector::new(vec![
            Ok(page(100_000_000, Vec::new(), false)),
            Ok(page(
                200_000_000,
                vec![PivotMatch {
                    seed: 226_061_463,
                    trial: 158_614_759,
                }],
                false,
            )),
        ]);
        let store = open_store(collector.clone());
        let job = finished_job(&store, &query_payload(4, "flat"));
        assert_eq!(job.state, "completed");
        assert_eq!(job.stop_reason, Some("result_limit"));
        assert_eq!(job.candidates.len(), 1);
        assert_eq!(job.candidates[0]["seed"], 226_061_463);
        assert_eq!(job.candidates[0]["cursor"], 158_614_759);
        assert_eq!(job.cursor, 200_000_000);
        assert_eq!(job.start_cursor, 0);
        assert!(job.resume_token.is_some());
        assert_eq!(
            collector.seen(),
            vec![(0, 100_000_000, 1), (100_000_000, 100_000_000, 1)],
            "each page gets the full result_count budget"
        );
    }

    #[test]
    fn both_rule_key_shapes_are_accepted() {
        for shape in ["flat", "groups"] {
            let collector = ScriptedCollector::new(vec![Ok(page(
                10,
                vec![PivotMatch { seed: 1, trial: 5 }],
                true,
            ))]);
            let store = open_store(collector);
            let job = finished_job(&store, &query_payload(4, shape));
            assert_eq!(job.state, "completed", "{shape}");
            assert_eq!(job.candidates.len(), 1, "{shape}");
        }
    }

    #[test]
    fn bounded_mode_still_stops_at_the_page_and_job_budget() {
        let collector = ramped_pages(10, 1_000_000);
        let store = open_store(collector.clone());
        let mut start = params(false);
        start.page_trials = 1_000_000;
        start.job_trials = 10_000_000;
        let job = wait_terminal(
            &store,
            &store
                .start(&query_payload(4, "flat"), &start)
                .expect("start")
                .job_id,
        );
        assert_eq!(job.stop_reason, Some("budget_reached"));
        assert!(job.candidates.is_empty());
        assert_eq!(job.cursor, 10_000_000);
        assert_eq!(collector.seen().len(), 10);

        // The page budget is clamped to what is left of the job budget.
        let collector = ScriptedCollector::new(vec![Ok(page(500_000, Vec::new(), false))]);
        let store = open_store(collector.clone());
        let mut start = params(false);
        start.page_trials = 1_000_000;
        start.job_trials = 500_000;
        let job = wait_terminal(
            &store,
            &store
                .start(&query_payload(4, "flat"), &start)
                .expect("start")
                .job_id,
        );
        assert_eq!(job.stop_reason, Some("budget_reached"));
        assert_eq!(collector.seen(), vec![(0, 500_000, 1)]);
    }

    /// A collector's native unit caps each page, so matches are published per
    /// unit; the covered trials and the final cursor are the same.
    #[test]
    fn pages_are_capped_at_the_native_unit_so_matches_stream() {
        struct Unit(Arc<ScriptedCollector>);
        impl SearchCollector for Unit {
            fn native_unit_trials(&self) -> Option<u64> {
                Some(250_000)
            }
            fn collect(
                &self,
                request: &BatchRequest<'_>,
                progress: &mut dyn FnMut(&IntersectionReport),
                cancelled: &dyn Fn() -> bool,
            ) -> Result<SearchBatch, CollectorError> {
                self.0.collect(request, progress, cancelled)
            }
        }
        let scripted = ScriptedCollector::new(vec![
            Ok(page(250_000, Vec::new(), false)),
            Ok(page(500_000, Vec::new(), false)),
        ]);
        let store = open_store_with(Some(Arc::new(FixedFactory(Arc::new(Unit(
            scripted.clone(),
        ))))));
        let mut start = params(false);
        start.page_trials = 1_000_000;
        start.job_trials = 500_000;
        let job = wait_terminal(
            &store,
            &store
                .start(&query_payload(4, "flat"), &start)
                .expect("start")
                .job_id,
        );
        assert_eq!(job.cursor, 500_000);
        assert_eq!(
            scripted
                .seen()
                .iter()
                .map(|(start, budget, _)| (*start, *budget))
                .collect::<Vec<_>>(),
            vec![(0, 250_000), (250_000, 250_000)]
        );
    }

    /// The job layer's half of the policy-restoration property: each job's own
    /// `allow_cpu_fallback` reaches every page of that job and nothing else, so
    /// a leaked opt-in cannot be hidden at this layer.
    #[test]
    fn the_execution_policy_flag_is_passed_per_job_and_per_page() {
        let collector = ScriptedCollector::new(vec![
            Ok(page(10, Vec::new(), false)),
            Ok(page(10, Vec::new(), false)),
        ]);
        let store = open_store(collector.clone());

        let mut opted_in = bounded_params();
        opted_in.allow_cpu_fallback = true;
        let first = store
            .start(&query_payload(4, "flat"), &opted_in)
            .expect("opted-in job starts");
        let _ = wait_terminal(&store, &first.job_id);

        let strict = store
            .start(&query_payload(4, "flat"), &bounded_params())
            .expect("strict job starts");
        let _ = wait_terminal(&store, &strict.job_id);

        assert_eq!(
            collector.policies(),
            vec![true, false],
            "the opted-in job must not change the next job's policy request"
        );
        assert_eq!(collector.seen().len(), 2, "one bounded page per job");
    }

    #[test]
    fn family_exhaustion_reports_no_resume_token() {
        let collector = ScriptedCollector::new(vec![Ok(page(1 << 20, Vec::new(), true))]);
        let store = open_store(collector);
        let job = finished_job(&store, &query_payload(4, "flat"));
        assert_eq!(job.stop_reason, Some("family_exhausted"));
        assert_eq!(job.resume_token, None);
    }

    #[test]
    fn overflow_and_bad_checkpoints_fail_closed() {
        let overflow = ScriptedCollector::new(vec![Ok(page(
            20,
            vec![
                PivotMatch { seed: 1, trial: 10 },
                PivotMatch { seed: 2, trial: 20 },
            ],
            false,
        ))]);
        let store = open_store(overflow);
        let job = finished_job(&store, &query_payload(4, "flat"));
        assert_eq!(job.state, "failed");
        assert_eq!(job.error.expect("error").0, "RESULT_OVERFLOW");

        let beyond = ScriptedCollector::new(vec![Ok(page(200, Vec::new(), false))]);
        let bounded = open_store(beyond);
        let mut start = params(true);
        start.page_trials = 100;
        let job = wait_terminal(
            &bounded,
            &bounded
                .start(&query_payload(4, "flat"), &start)
                .expect("start")
                .job_id,
        );
        assert_eq!(job.error.expect("error").0, "INVALID_CHECKPOINT");
    }

    #[test]
    fn a_missing_checkpoint_and_no_progress_fail_closed() {
        let missing = ScriptedCollector::new(vec![Ok(SearchBatch::default())]);
        let store = open_store(missing);
        let job = finished_job(&store, &query_payload(4, "flat"));
        assert_eq!(job.error.expect("error").0, "INVALID_CHECKPOINT");

        let stuck = ScriptedCollector::new(vec![
            Ok(page(10, Vec::new(), false)),
            Ok(page(10, Vec::new(), false)),
        ]);
        let stuck_store = open_store(stuck);
        let job = finished_job(&stuck_store, &query_payload(4, "flat"));
        assert_eq!(job.error.expect("error").0, "NO_PROGRESS");
    }

    #[test]
    fn a_backend_error_fails_the_job_with_its_own_code() {
        let collector = ScriptedCollector::new(vec![Err(CollectorError::new(
            "SEARCH_FAILED",
            "DirectCompute partial-effect matcher is unavailable; CPU fallback is disabled",
        ))]);
        let store = open_store(collector);
        let job = finished_job(&store, &query_payload(4, "flat"));
        assert_eq!(job.state, "failed");
        assert_eq!(job.stop_reason, Some("error"));
        let (code, message) = job.error.expect("error");
        assert_eq!(code, "SEARCH_FAILED");
        assert!(message.contains("CPU fallback is disabled"));
        assert_eq!(job.resume_token, None);
    }

    #[test]
    fn a_missing_collector_reports_search_backend_unavailable() {
        let store = open_store_with(None);
        let job = finished_job(&store, &query_payload(4, "flat"));
        assert_eq!(job.state, "failed");
        let (code, message) = job.error.expect("error");
        assert_eq!(code, "SEARCH_BACKEND_UNAVAILABLE");
        assert!(message.contains("bounded native pivot collector"));
    }

    #[test]
    fn an_unpackable_filter_fails_the_request_and_names_itself() {
        let store = open_store_with(Some(Arc::new(RejectingFactory("rarity-5 wildcard"))));
        let error = store
            .start(&query_payload(4, "flat"), &params(true))
            .expect_err("compilation rejection");
        assert_eq!(error.code, "INVALID_REQUEST");
        assert!(
            error.message.contains("rarity-5 wildcard"),
            "the message must name the filter, got {}",
            error.message
        );
    }

    /// A backend that cannot run keeps its own code through start-time rejection.
    #[test]
    fn an_unavailable_backend_keeps_its_own_code_at_start() {
        struct UnavailableFactory;

        impl SearchFactory for UnavailableFactory {
            fn collector(
                &self,
                _query: &SearchQuery,
            ) -> Result<Arc<dyn SearchCollector>, CollectorError> {
                Err(CollectorError::unavailable(
                    "the native seed accelerator is unavailable, so bounded search cannot run",
                ))
            }
        }

        let store = open_store_with(Some(Arc::new(UnavailableFactory)));
        let error = store
            .start(&query_payload(4, "flat"), &params(true))
            .expect_err("unavailable backend");
        assert_eq!(error.code, "SEARCH_BACKEND_UNAVAILABLE");
        assert!(error.message.contains("accelerator is unavailable"));
    }

    /// The post-acceptance enemy-occurrence filter must really reject, never be
    /// assumed to have been applied by the native pivot.
    #[test]
    fn an_unverified_enemy_occurrence_match_rejects_the_candidate() {
        for verdict in [Some(false), None] {
            let scripted = ScriptedCollector::new(vec![Ok(page(
                10,
                vec![PivotMatch { seed: 7, trial: 3 }],
                true,
            ))]);
            let factory: Arc<dyn SearchFactory> = Arc::new(FixedFactory(scripted));
            let source: Arc<dyn CandidateSource> = Arc::new(FakeSource {
                enemy_occurrence_match: verdict,
                auxiliary_match: Some(true),
            });
            let store = Arc::new(
                JobStore::new(CONTEXT, Some(factory), source).expect("an OS CSPRNG is available"),
            );
            let job = finished_job(&store, &query_payload(4, "flat"));
            assert_eq!(job.state, "completed", "verdict {verdict:?}");
            assert_eq!(job.stop_reason, Some("family_exhausted"));
            assert!(
                job.candidates.is_empty(),
                "verdict {verdict:?} must drop the candidate"
            );
            assert_eq!(job.cursor, 10, "the page checkpoint still advances");
        }
    }

    /// The post-acceptance auxiliary-criteria filter must really reject. This is
    /// the check that stops the R4-primary route from publishing candidates that
    /// violate terrain / rule / enemy criteria its pivot never narrowed on.
    #[test]
    fn an_auxiliary_criteria_mismatch_rejects_the_candidate() {
        for verdict in [Some(false), None] {
            let scripted = ScriptedCollector::new(vec![Ok(page(
                10,
                vec![PivotMatch { seed: 7, trial: 3 }],
                true,
            ))]);
            let factory: Arc<dyn SearchFactory> = Arc::new(FixedFactory(scripted));
            let source: Arc<dyn CandidateSource> = Arc::new(FakeSource {
                enemy_occurrence_match: Some(true),
                auxiliary_match: verdict,
            });
            let store = Arc::new(
                JobStore::new(CONTEXT, Some(factory), source).expect("an OS CSPRNG is available"),
            );
            let job = finished_job(&store, &query_payload(4, "flat"));
            assert_eq!(job.stop_reason, Some("family_exhausted"));
            assert!(
                job.candidates.is_empty(),
                "verdict {verdict:?} must drop the candidate"
            );
            assert_eq!(job.cursor, 10, "the page checkpoint still advances");
        }
    }

    #[test]
    fn cancel_is_observed_inside_a_long_page_and_keeps_the_checkpoint() {
        let collector = GatedCollector::new();
        let store = open_store(collector.clone());
        let started = store
            .start(&query_payload(4, "flat"), &params(true))
            .expect("start accepted");
        wait_for(|| {
            !matches!(
                store.snapshot(&started.job_id).expect("job").state,
                "queued"
            )
        });
        let cancelled = store.cancel(&started.job_id).expect("cancel accepted");
        assert_eq!(cancelled.state, "cancel_requested");
        collector.release();
        let job = wait_terminal(&store, &started.job_id);
        assert_eq!(job.state, "cancelled");
        assert_eq!(job.stop_reason, Some("cancelled"));
        assert_eq!(job.cursor, 16);
        assert!(job.resume_token.is_some());
        assert!(job.elapsed_ms < 30_000);
    }

    /// The BUSY guard's real invariant: two `start` calls that are genuinely
    /// concurrent must still admit exactly one job.
    ///
    /// This is the deterministic version of the operator-visible symptom. The
    /// framed main loop dispatches one request at a time, so the only path to
    /// concurrent `start` calls is a caller that issues them in parallel, which
    /// is what the two threads below do. The collector waits for an explicit
    /// release after both starts return, so scheduling cannot end the first
    /// job underneath the race.
    #[test]
    fn concurrent_starts_admit_exactly_one_job() {
        const RACES: usize = 128;
        let payload = query_payload(4, "flat");
        let mut refusals = 0usize;
        for _ in 0..RACES {
            let collector = GatedCollector::new();
            let store = open_store(collector.clone());
            let barrier = Arc::new(std::sync::Barrier::new(2));
            let mut handles = Vec::new();
            for _ in 0..2 {
                let store = Arc::clone(&store);
                let payload = payload.clone();
                let barrier = Arc::clone(&barrier);
                handles.push(thread::spawn(move || {
                    barrier.wait();
                    store.start(&payload, &params(true))
                }));
            }
            let outcomes: Vec<_> = handles
                .into_iter()
                .map(|handle| handle.join().expect("start thread"))
                .collect();
            collector.release();
            let mut accepted = Vec::new();
            let mut refused = Vec::new();
            for outcome in outcomes {
                match outcome {
                    Ok(job) => accepted.push(job),
                    Err(error) => refused.push(error.code),
                }
            }
            assert_eq!(
                (accepted.len(), refused.len()),
                (1, 1),
                "exactly one concurrent start may own the worker"
            );
            assert_eq!(
                refused[0], "BUSY",
                "the loser must be refused by name, not replaced silently"
            );
            refusals += 1;
            // The surviving owner is the one the store actually reports, so a
            // second thread cannot have replaced the accepted job's record.
            let current = store.current().expect("one job survives");
            assert_eq!(current.job_id, accepted[0].job_id);
            wait_terminal(&store, &current.job_id);
            // Publishing terminal state precedes the owner's thread exit.
            wait_for(|| !store.is_alive());
            assert!(!store.is_alive(), "the job thread ends with its job");
        }
        assert_eq!(refusals, RACES, "every race produced exactly one BUSY");
    }

    #[test]
    fn cancel_and_snapshot_report_job_not_found_for_unknown_ids() {
        let store = open_store(ScriptedCollector::new(Vec::new()));
        assert_eq!(
            store.cancel("missing").expect_err("unknown job").code,
            "JOB_NOT_FOUND"
        );
        assert_eq!(
            store.snapshot("missing").expect_err("unknown job").code,
            "JOB_NOT_FOUND"
        );
        assert!(store.current().is_none());
    }

    #[test]
    fn busy_and_context_mismatch_are_reported_before_starting() {
        let collector = GatedCollector::new();
        let store = open_store(collector.clone());
        let mut mismatch = params(true);
        mismatch.context_digest = "0".repeat(64);
        assert_eq!(
            store
                .start(&query_payload(4, "flat"), &mismatch)
                .expect_err("context mismatch")
                .code,
            "CONTEXT_MISMATCH"
        );
        let started = store
            .start(&query_payload(4, "flat"), &params(true))
            .expect("start accepted");
        assert_eq!(
            store
                .start(&query_payload(4, "flat"), &params(true))
                .expect_err("busy")
                .code,
            "BUSY"
        );
        collector.release();
        let _ = wait_terminal(&store, &started.job_id);
    }

    #[test]
    fn ng4_and_cache_requests_fail_closed() {
        let store = open_store(ScriptedCollector::new(Vec::new()));
        let mut payload = query_payload(5, "flat");
        payload["playthrough"] = json!(5);
        assert_eq!(
            store.start(&payload, &params(true)).expect_err("ng5").code,
            "INVALID_REQUEST"
        );
        let mut with_cache = params(true);
        with_cache.cache_id = Some("a".repeat(64));
        assert_eq!(
            store
                .start(&query_payload(4, "flat"), &with_cache)
                .expect_err("ng3 cache")
                .code,
            "INVALID_REQUEST"
        );
    }

    #[test]
    fn export_requires_the_owning_job_and_a_retained_candidate() {
        let collector = ScriptedCollector::new(vec![Ok(page(
            10,
            vec![PivotMatch { seed: 7, trial: 3 }],
            true,
        ))]);
        let store = open_store(collector);
        let job = finished_job(&store, &query_payload(4, "flat"));
        let candidate_id = job.candidates[0]["candidate_id"]
            .as_str()
            .expect("candidate id")
            .to_string();
        let transfer = store
            .export(&job.job_id, &candidate_id)
            .expect("the owning job exports its candidate");
        assert_eq!(transfer["candidate_id"], candidate_id);
        assert_eq!(transfer["context_digest"], CONTEXT);
        assert_eq!(transfer["level"], 180);
        assert_eq!(
            store
                .export("other-job", &candidate_id)
                .expect_err("wrong job")
                .code,
            "JOB_NOT_FOUND"
        );
        assert_eq!(
            store
                .export(&job.job_id, &"f".repeat(64))
                .expect_err("foreign candidate")
                .code,
            "INVALID_REQUEST"
        );
    }

    #[test]
    fn snapshots_are_detached_from_the_live_job() {
        let collector = GatedCollector::new();
        let store = open_store(collector.clone());
        let started = store
            .start(&query_payload(4, "flat"), &params(true))
            .expect("start accepted");
        let first = store.snapshot(&started.job_id).expect("snapshot");
        collector.release();
        let job = wait_terminal(&store, &started.job_id);
        assert!(first.candidates.is_empty());
        assert_eq!(job.candidates.len(), 1);
        assert_ne!(first.sequence, job.sequence);
        assert_ne!(first.state, job.state);
    }

    #[test]
    fn resume_tokens_are_bound_to_query_context_policy_and_continuation() {
        let first = open_store(ramped_pages(1, 10));
        let started = first
            .start(&query_payload(4, "flat"), &bounded_params())
            .expect("start accepted");
        let job = wait_terminal(&first, &started.job_id);
        assert_eq!(job.stop_reason, Some("budget_reached"));
        let token = job.resume_token.clone().expect("token minted");
        let resumed = first
            .start(&query_payload(4, "flat"), &resume_bounded(&token))
            .expect("the same binding resumes");
        assert_eq!(resumed.start_cursor, 10);
        let _ = wait_terminal(&first, &resumed.job_id);

        let rebound = open_store(ramped_pages(1, 10));
        let started = rebound
            .start(&query_payload(4, "flat"), &bounded_params())
            .expect("start accepted");
        let job = wait_terminal(&rebound, &started.job_id);
        let token = job.resume_token.clone().expect("token minted");

        let mut changed_policy = resume_bounded(&token);
        changed_policy.allow_cpu_fallback = true;
        assert_eq!(
            rebound
                .start(&query_payload(4, "flat"), &changed_policy)
                .expect_err("changed policy")
                .code,
            "INVALID_RESUME_TOKEN"
        );
        let mut changed_continuation = resume_bounded(&token);
        changed_continuation.continue_until_complete = true;
        assert_eq!(
            rebound
                .start(&query_payload(4, "flat"), &changed_continuation)
                .expect_err("changed continuation")
                .code,
            "INVALID_RESUME_TOKEN"
        );
        let mut changed_context = resume_bounded(&token);
        changed_context.context_digest = "1".repeat(64);
        assert_eq!(
            rebound
                .start(&query_payload(4, "flat"), &changed_context)
                .expect_err("changed context")
                .code,
            "CONTEXT_MISMATCH"
        );
        assert_eq!(
            rebound
                .start(&query_payload(4, "groups"), &resume_bounded(&token))
                .expect_err("changed query")
                .code,
            "INVALID_RESUME_TOKEN"
        );
        assert_eq!(
            rebound
                .start(
                    &query_payload(4, "flat"),
                    &resume_bounded(&format!("{token}00")),
                )
                .expect_err("forged signature")
                .code,
            "INVALID_RESUME_TOKEN"
        );
        assert_eq!(
            rebound
                .start(&query_payload(4, "flat"), &resume_bounded("not-a-token"))
                .expect_err("garbage token")
                .code,
            "INVALID_RESUME_TOKEN"
        );
        assert_eq!(
            rebound
                .start(&query_payload(4, "flat"), &resume_bounded("Zm9v.Zm9v"))
                .expect_err("re-encoded body")
                .code,
            "INVALID_RESUME_TOKEN"
        );
    }

    #[test]
    fn a_token_from_another_session_is_rejected() {
        let origin = open_store(ramped_pages(1, 10));
        let started = origin
            .start(&query_payload(4, "flat"), &bounded_params())
            .expect("start accepted");
        let job = wait_terminal(&origin, &started.job_id);
        let token = job.resume_token.clone().expect("token minted");
        let restarted = open_store(ramped_pages(1, 10));
        assert_eq!(
            restarted
                .start(&query_payload(4, "flat"), &resume_bounded(&token))
                .expect_err("another session secret")
                .code,
            "INVALID_RESUME_TOKEN"
        );
    }

    /// Bounded mode ends at `job_trials`, the only path that publishes a resume
    /// token without exhausting the pivot family.
    fn bounded_params() -> StartParams {
        let mut params = params(false);
        params.page_trials = 10;
        params.job_trials = 10;
        params
    }

    fn resume_bounded(token: &str) -> StartParams {
        let mut params = bounded_params();
        params.resume_token = Some(token.to_string());
        params
    }

    #[test]
    fn hmac_matches_rfc_4231_test_vectors() {
        let key = [0x0bu8; 20];
        assert_eq!(
            hex_lower(&hmac_sha256(&key, b"Hi There")),
            "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7"
        );
        assert_eq!(
            hex_lower(&hmac_sha256(b"Jefe", b"what do ya want for nothing?")),
            "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
        );
        let long_key = [0xaau8; 131];
        assert_eq!(
            hex_lower(&hmac_sha256(
                &long_key,
                b"Test Using Larger Than Block-Size Key - Hash Key First"
            )),
            "60e431591ee0b67f0d8a26aacbf5b77f8e0bc6213728c5140546040f0ee37f54"
        );
    }

    #[test]
    fn base64url_round_trips_and_matches_python_padding() {
        assert_eq!(base64url_encode(b""), "");
        assert_eq!(base64url_encode(b"f"), "Zg==");
        assert_eq!(base64url_encode(b"fo"), "Zm8=");
        assert_eq!(base64url_encode(b"foo"), "Zm9v");
        for sample in [
            &b""[..],
            b"f",
            b"fo",
            b"foo",
            b"foobar",
            &[0xff, 0xfe, 0xfd],
        ] {
            assert_eq!(
                base64url_decode(&base64url_encode(sample)).expect("decodes"),
                sample
            );
        }
        assert!(base64url_decode("A").is_none());
    }

    #[test]
    fn uuid4_has_the_shipped_shape_and_version_bits() {
        let value = entropy::uuid4();
        assert_eq!(value.len(), 36);
        assert_eq!(value.as_bytes()[8], b'-');
        assert_eq!(value.as_bytes()[13], b'-');
        assert_eq!(value.as_bytes()[14], b'4');
        assert_eq!(value.as_bytes()[18], b'-');
        assert_eq!(value.as_bytes()[23], b'-');
        assert!(value
            .chars()
            .all(|character| character == '-' || character.is_ascii_hexdigit()));
        assert_ne!(value, entropy::uuid4());
    }

    #[test]
    fn the_binding_uses_pythons_boolean_and_none_spellings() {
        assert_eq!(
            binding("digest", CONTEXT, false, true, None),
            format!("digest:{CONTEXT}:False:True:None")
        );
        assert_eq!(
            binding("digest", CONTEXT, true, false, Some("cache")),
            format!("digest:{CONTEXT}:True:False:cache")
        );
    }

    fn grace_effect(slot: u32, effect_id: u32, effect_flags: u32) -> CandidateEffect {
        CandidateEffect {
            slot,
            effect_id,
            value: 1,
            metadata: effect_flags << 16,
            prefix: 0,
            tail_0: 0,
            tail_1: 0,
            roll_percent: Some(0),
        }
    }

    fn graced(rarity: u8, effects: Vec<CandidateEffect>) -> MaterializedCandidate {
        MaterializedCandidate {
            candidate: Candidate {
                seed: 1,
                playthrough: Some(3),
                rarity,
                record_stage: RecordStage::EffectSequenceOnly,
                record: Vec::new(),
                installation_record: None,
                effects,
                joint_search_trial: Some(1),
            },
            payload: Value::Null,
            auxiliary_match: Some(true),
            enemy_occurrence_match: Some(true),
        }
    }

    /// `c.grace is not None and c.grace.effect_id in query.grace_effect_ids`:
    /// the rarity-5 Grace is the sixth effect; a rarity-4 Grace exists only when
    /// the fifth effect survived as a verified final Grace with the fixed bit.
    #[test]
    fn the_final_grace_filter_matches_the_shipped_candidate_grace() {
        let query = |rarity: u8, graces: Value| {
            let mut payload = query_payload(rarity, "flat");
            payload["grace_effect_ids"] = graces;
            SearchQuery::from_payload(&payload).expect("valid")
        };
        let ordinary = |slot| grace_effect(slot, 0x1000 + slot, 0);

        let r5 = query(5, json!([0x6553, 0xCE68]));
        let mut effects: Vec<CandidateEffect> = (1..=5).map(ordinary).collect();
        effects.push(grace_effect(6, 0xCE68, 0x02));
        assert!(accepts(&r5, &graced(5, effects.clone())));
        effects[5].effect_id = 0xBABD;
        assert!(!accepts(&r5, &graced(5, effects)));
        assert!(
            !accepts(&r5, &graced(5, (1..=5).map(ordinary).collect())),
            "a candidate with no Grace never matches a Grace selection"
        );

        let r4 = query(4, json!([0x6553]));
        let mut effects: Vec<CandidateEffect> = (1..=4).map(ordinary).collect();
        effects.push(grace_effect(5, 0x6553, 0x02));
        assert!(accepts(&r4, &graced(4, effects.clone())));
        effects[4].metadata = 0;
        assert!(
            !accepts(&r4, &graced(4, effects)),
            "a slot the finalizer replaced is not a Grace even if the id coincides"
        );
        // An ordinary effect that happens to carry the id elsewhere is not a
        // Grace either.
        let mut effects: Vec<CandidateEffect> = vec![grace_effect(1, 0x6553, 0x02)];
        effects.extend((2..=5).map(ordinary));
        assert!(!accepts(&r4, &graced(4, effects)));
    }
}
