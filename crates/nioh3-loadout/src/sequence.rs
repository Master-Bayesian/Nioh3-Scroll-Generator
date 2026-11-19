//! Resolved ordinary-resource sequences. Occurrences are ordered and may reuse
//! the same contact template; reuse never removes an occurrence or reassociates
//! floating-point addition. Runtime reference/charge/spawn admission is upstream.

#[path = "event_inputs.rs"]
pub mod inputs;

#[derive(Clone, Copy, Debug)]
pub struct ActionStep<'a> {
    pub action: u16,
    pub base_cost: f32,
    pub contacts: &'a [usize],
}

impl<'a> ActionStep<'a> {
    /// PC202 action-parameter i16+16, read by original4F54C4.
    pub fn from_parameter(
        action: u16,
        parameter: &[u8],
        contacts: &'a [usize],
    ) -> Result<Self, &'static str> {
        if parameter.len() != 336 {
            return Err("incomplete action parameter");
        }
        if u16::from_le_bytes([parameter[0xC], parameter[0xD]]) != action {
            return Err("action parameter belongs to another action");
        }
        Ok(Self {
            action,
            base_cost: i16::from_le_bytes([parameter[0x16], parameter[0x17]]) as f32,
            contacts,
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ResolvedSequence<'a> {
    pub entry: &'a [ActionStep<'a>],
    pub cycle: &'a [ActionStep<'a>],
    /// None: repeat until Ki is nonpositive. Some(n): execute at most n cycles.
    pub cycles: Option<u32>,
}

#[derive(Clone, Copy, Debug)]
pub struct ContactState {
    pub action: u16,
    pub action_ordinal: u32,
    pub template: usize,
    pub contact_ordinal: u32,
    pub ki_before: f32,
    pub ki_after: f32,
}

/// A resolved action debits once before its contacts. Positive cost admission
/// needs positive Ki, not sufficient Ki for the full debit (original49B108).
/// Special-resource gates, event-driven recovery and next-hit rebuilds are not
/// inferred here. The callback receives actual per-action resource state.
pub fn walk_sequence(
    plan: ResolvedSequence<'_>,
    maximum: f32,
    mut cost: impl FnMut(f32, f32, f32) -> Result<f32, &'static str>,
    mut visit: impl FnMut(ContactState) -> Result<(), &'static str>,
) -> Result<u32, &'static str> {
    if !maximum.is_finite() || maximum < 0.0 {
        return Err("invalid initial maximum Ki");
    }
    let mut ki = maximum;
    let mut contacts = 0u32;
    let mut actions = 0u32;
    let mut run = |step: &ActionStep<'_>| -> Result<bool, &'static str> {
        if !step.base_cost.is_finite() {
            return Err("invalid action base cost");
        }
        if step.base_cost > 0.0 && ki <= 0.0 {
            return Ok(false);
        }
        let debit = cost(step.base_cost, ki, maximum)?;
        let before = ki;
        ki -= debit;
        if !debit.is_finite() || !ki.is_finite() {
            return Err("nonfinite sequence debit");
        }
        for &template in step.contacts {
            visit(ContactState {
                action: step.action,
                action_ordinal: actions,
                template,
                contact_ordinal: contacts,
                ki_before: before,
                ki_after: ki,
            })?;
            contacts = contacts
                .checked_add(1)
                .ok_or("sequence contact count exceeds the result domain")?;
        }
        actions = actions
            .checked_add(1)
            .ok_or("sequence action count exceeds the result domain")?;
        Ok(true)
    };
    for step in plan.entry {
        if !run(step)? {
            return Ok(contacts);
        }
    }
    // Release the closure's borrow before each cycle progress check.
    drop(run);
    let mut cycles = 0u32;
    while !plan.cycle.is_empty()
        && plan.cycles.is_none_or(|n| cycles < n)
        && (plan.cycles.is_some() || ki > 0.0)
    {
        let before_cycle = ki;
        for step in plan.cycle {
            if !step.base_cost.is_finite() {
                return Err("invalid action base cost");
            }
            if step.base_cost > 0.0 && ki <= 0.0 {
                return Ok(contacts);
            }
            let before = ki;
            let debit = cost(step.base_cost, ki, maximum)?;
            ki -= debit;
            if !debit.is_finite() || !ki.is_finite() {
                return Err("nonfinite sequence debit");
            }
            for &template in step.contacts {
                visit(ContactState {
                    action: step.action,
                    action_ordinal: actions,
                    template,
                    contact_ordinal: contacts,
                    ki_before: before,
                    ki_after: ki,
                })?;
                contacts = contacts
                    .checked_add(1)
                    .ok_or("sequence contact count exceeds the result domain")?;
            }
            actions = actions
                .checked_add(1)
                .ok_or("sequence action count exceeds the result domain")?;
        }
        cycles = cycles
            .checked_add(1)
            .ok_or("sequence cycle count exceeds the result domain")?;
        if plan.cycles.is_none() && ki >= before_cycle {
            return Err("unbounded sequence has no proven finite Ki depletion");
        }
    }
    Ok(contacts)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parameter_cost_is_signed_as_in_original4f54c4() {
        let mut parameter = [0u8; 336];
        parameter[0xC..0xE].copy_from_slice(&9u16.to_le_bytes());
        parameter[0x16..0x18].copy_from_slice(&(-1i16).to_le_bytes());
        assert_eq!(
            ActionStep::from_parameter(9, &parameter, &[])
                .unwrap()
                .base_cost,
            -1.0
        );
        assert!(ActionStep::from_parameter(9, &parameter[..335], &[]).is_err());
        assert!(ActionStep::from_parameter(10, &parameter, &[]).is_err());
    }
    #[test]
    fn finite_and_alternating_plans_preserve_contacts_and_allow_last_overspend() {
        let entry = [ActionStep {
            action: 1,
            base_cost: 2.0,
            contacts: &[0, 0],
        }];
        let cycle = [
            ActionStep {
                action: 2,
                base_cost: 2.0,
                contacts: &[1],
            },
            ActionStep {
                action: 3,
                base_cost: 3.0,
                contacts: &[2, 3],
            },
        ];
        let mut seen = Vec::new();
        let count = walk_sequence(
            ResolvedSequence {
                entry: &entry,
                cycle: &cycle,
                cycles: None,
            },
            6.0,
            |base, _, _| Ok(base),
            |hit| {
                seen.push((hit.template, hit.ki_after));
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(count, 5);
        assert_eq!(
            seen,
            vec![(0, 4.0), (0, 4.0), (1, 2.0), (2, -1.0), (3, -1.0)]
        );
    }
    #[test]
    fn zero_cost_finite_plan_is_valid_but_unbounded_nonprogress_is_not_capped() {
        let cycle = [ActionStep {
            action: 9,
            base_cost: 0.0,
            contacts: &[0],
        }];
        let plan = ResolvedSequence {
            entry: &[],
            cycle: &cycle,
            cycles: Some(7),
        };
        assert_eq!(walk_sequence(plan, 1.0, |v, _, _| Ok(v), |_| Ok(())), Ok(7));
        assert!(walk_sequence(
            ResolvedSequence {
                cycles: None,
                ..plan
            },
            1.0,
            |v, _, _| Ok(v),
            |_| Ok(())
        )
        .is_err());
    }
    #[test]
    fn contacts_keep_their_binary32_sum_order() {
        let entry = [ActionStep {
            action: 8,
            base_cost: 0.0,
            contacts: &[0, 1, 1],
        }];
        let values = [16777216.0f32, 1.0f32];
        let mut sum = 0.0f32;
        walk_sequence(
            ResolvedSequence {
                entry: &entry,
                cycle: &[],
                cycles: Some(0),
            },
            0.0,
            |v, _, _| Ok(v),
            |hit| {
                sum += values[hit.template];
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(sum, 16777216.0);
        assert_ne!(sum, values[0] + values[1] * 2.0);
    }
}
