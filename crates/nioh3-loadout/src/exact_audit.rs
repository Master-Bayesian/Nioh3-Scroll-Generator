//! Explicit retained-corpus acceptance. This is deliberately expensive and is
//! never part of timed production search or a substitute for original MAIN.
use super::*;
use crate::damage::DamageWords;
use crate::gpu::MainCpu;

#[derive(Clone, Debug, Serialize)]
pub struct ExactAuditRow {
    pub ordinal: u64,
    pub items: [Option<u32>; crate::engine::SLOTS],
    pub attrs: [i32; 7],
    pub outcome: serde_json::Value,
    pub cpu_components: Option<DamageWords>,
    pub cuda_components: Option<DamageWords>,
    pub cpu_certificate: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct ExactAuditBatch {
    pub from: u64,
    pub compared: u64,
    pub answered: u64,
    pub outside_limits: u64,
    pub unanswered: u64,
    pub rows: Vec<ExactAuditRow>,
}

pub struct ExactAudit<'s, 'a> {
    sweep: &'s ExactSweep<'a>,
    reference_work: Work,
    fused_work: Work,
    cpu: MainCpu,
    device: Option<MainDevice>,
    bounds: Option<ReadBounds>,
}

fn outcome_json(outcome: Outcome) -> serde_json::Value {
    match outcome {
        Outcome::Damage(bits) => serde_json::json!({"state":"damage", "bits":bits}),
        Outcome::OutsideLimits => serde_json::json!({"state":"outside_limits"}),
        Outcome::NoPath => serde_json::json!({"state":"no_path"}),
        Outcome::Refused(reason) => serde_json::json!({"state":"refused", "reason":reason}),
    }
}

fn add_components(sum: &mut DamageWords, value: DamageWords) -> Result<(), String> {
    if value.reserved != 0 {
        return Err("invalid MAIN component payload".into());
    }
    for (total, part) in [
        (&mut sum.total, value.total),
        (&mut sum.physical, value.physical),
        (&mut sum.elemental, value.elemental),
    ] {
        let next = f32::from_bits(*total) + f32::from_bits(part);
        if !next.is_finite() {
            return Err("non-finite MAIN channel sum".into());
        }
        *total = next.to_bits();
    }
    Ok(())
}

impl<'a> ExactSweep<'a> {
    /// Compare the original two-pass preparation and direct CPU score against
    /// shared preparation + CPU certificates, then compare all component bits
    /// on identical strict CPU/CUDA packets. Independent of search timing.
    pub fn audit_session(&self, compare_cuda: bool) -> Result<ExactAudit<'_, 'a>, String> {
        if !self.search.engine.supports_prepared_device() {
            return Err("prepared audit requires a captured fixed, scored contacts".into());
        }
        Ok(ExactAudit {
            sweep: self,
            reference_work: self.work()?,
            fused_work: self.work()?,
            cpu: self.search.engine.open_prepared_cpu()?,
            device: compare_cuda
                .then(|| self.search.engine.open_prepared_device())
                .transpose()?,
            bounds: self
                .search
                .engine
                .supports_read_bounds()
                .then(ReadBounds::new),
        })
    }
}

impl ExactAudit<'_, '_> {
    /// Direct-device control: every retained packet runs natively when disabled.
    pub fn set_device_reuse_enabled(&mut self, enabled: bool) {
        if let Some(device) = &mut self.device {
            device.set_reuse_enabled(enabled);
        }
    }

    pub fn batch(&mut self, from: u64, count: usize) -> Result<ExactAuditBatch, String> {
        if !(1..=4096).contains(&count) || from > self.sweep.identity.planned {
            return Err("invalid audit batch range".into());
        }
        let chunk = (4096 / self.sweep.search.engine.hit_count().max(1)).max(1);
        let first = count.min(chunk);
        let mut total = self.batch_inner(from, first)?;
        for offset in (first..count).step_by(chunk) {
            if from.saturating_add(offset as u64) >= self.sweep.identity.planned {
                break;
            }
            let more = self.batch_inner(from + offset as u64, (count - offset).min(chunk))?;
            total.compared += more.compared;
            total.answered += more.answered;
            total.outside_limits += more.outside_limits;
            total.unanswered += more.unanswered;
            total.rows.extend(more.rows);
        }
        Ok(total)
    }

    fn batch_inner(&mut self, from: u64, count: usize) -> Result<ExactAuditBatch, String> {
        let end = from
            .saturating_add(count as u64)
            .min(self.sweep.identity.planned);
        let mut report = ExactAuditBatch {
            from,
            compared: 0,
            answered: 0,
            outside_limits: 0,
            unanswered: 0,
            rows: Vec::with_capacity((end - from) as usize),
        };
        let search = &self.sweep.search;
        let engine = search.engine;
        let request = &self.sweep.request;
        let mut contacts = Vec::new();
        let mut lanes = Vec::new();
        for ordinal in from..end {
            let candidate = self.sweep.candidate(ordinal);
            // Frozen algorithmic reference: the native set pipeline, followed
            // by the original independent eligible preparation and score.
            let admission = search.set_admission(&candidate, request, &mut self.reference_work);
            let reference = match admission {
                Ok(()) => engine.evaluate(
                    &candidate,
                    search.pack,
                    &request.limits,
                    &mut self.reference_work,
                ),
                Err(outcome) => outcome,
            };
            let mut certified = false;
            let fused = match search.prepare_candidate(&candidate, request, &mut self.fused_work) {
                Err(outcome) => outcome,
                Ok(prepared) => match &mut self.bounds {
                    Some(bounds) => match prepared.evaluate_with_bound(bounds) {
                        SweepOutcome::Executed(outcome) => outcome,
                        SweepOutcome::Certified(bits) => {
                            certified = true;
                            Outcome::Damage(bits)
                        }
                    },
                    None => prepared.evaluate(),
                },
            };
            if reference != fused {
                return Err(format!("audit outcome mismatch at ordinal {ordinal}: reference={reference:?}, shared={fused:?}"));
            }
            let reference_words = admission.and_then(|()| {
                engine.prepare_device_contacts(
                    &candidate,
                    search.pack,
                    &request.limits,
                    &mut self.reference_work,
                )
            });
            let fused_words = search
                .prepare_candidate(&candidate, request, &mut self.fused_work)
                .and_then(|prepared| prepared.device_contacts());
            if reference_words != fused_words {
                return Err(format!(
                    "audit ordered prepared-input mismatch at ordinal {ordinal}"
                ));
            }
            let mut row = ExactAuditRow {
                ordinal,
                items: candidate.items,
                attrs: candidate.attrs,
                outcome: outcome_json(fused),
                cpu_components: None,
                cuda_components: None,
                cpu_certificate: certified,
            };
            match fused {
                Outcome::Damage(bits) if canonical_score_bits(bits).is_some() => {
                    report.answered += 1;
                    let words = reference_words.map_err(|outcome| format!(
                        "audit answered candidate failed input preparation at {ordinal}: {outcome:?}"
                    ))?;
                    let mut components = DamageWords::default();
                    for packet in words {
                        let value = self.cpu.run_components(&packet).map_err(|error| {
                            format!("audit strict CPU components failed at {ordinal}: {error:?}")
                        })?;
                        add_components(&mut components, value)?;
                        if self.device.is_some() {
                            lanes.push((report.rows.len(), value));
                            contacts.push(packet);
                        }
                    }
                    if self.device.is_some() {
                        row.cuda_components = Some(DamageWords::default());
                    }
                    if components.reserved != 0
                        || [components.total, components.physical, components.elemental]
                            .iter()
                            .any(|bits| !f32::from_bits(*bits).is_finite())
                        || (0.0f32 + f32::from_bits(components.total)).to_bits() != bits
                    {
                        return Err(format!(
                            "audit invalid CPU component/total at ordinal {ordinal}"
                        ));
                    }
                    row.cpu_components = Some(components);
                }
                Outcome::OutsideLimits => report.outside_limits += 1,
                _ => report.unanswered += 1,
            }
            report.rows.push(row);
        }
        if let Some(device) = &mut self.device {
            let scores = device.run(&contacts)?;
            if scores.len() != lanes.len() {
                return Err("audit device returned incomplete component results".into());
            }
            for ((index, expected), score) in lanes.into_iter().zip(scores) {
                let row = &mut report.rows[index];
                if score.bail != 0 || score.damage != expected {
                    return Err(format!(
                        "audit strict CPU/CUDA component mismatch at ordinal {}",
                        row.ordinal
                    ));
                }
                add_components(
                    row.cuda_components
                        .as_mut()
                        .ok_or("audit candidate missing CUDA result")?,
                    score.damage,
                )?;
            }
        }
        if self.device.is_some()
            && report
                .rows
                .iter()
                .any(|row| row.cuda_components != row.cpu_components)
        {
            return Err("audit per-candidate CUDA channel sums differ from CPU".into());
        }
        report.compared = end - from;
        Ok(report)
    }
}
