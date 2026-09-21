# Diagnose

Read the failed stage, not the whole repository. Capture the full source SHA,
run ID, step, error, preceding successful stages and retained artifact paths.

Classify the cause before selecting a repair:

| Cause | Next action |
| --- | --- |
| Product behavior or artifact identity | Repair the implementation, verify affected behavior, freeze a new candidate. |
| Runner prerequisite or harness | Reproduce against the retained candidate; fix setup or observation. Preserve meaningful assertions and label the evidence scope. |
| Unbounded workload on CPU CI | Move the expensive investigation to the explicit extended lane; use direct known-seed and bounded workflow checks for release. |
| Asynchronous observation race | Observe protocol responses, terminal reports and actual process completion instead of tiny scheduling windows. |
| Network or interrupted observation | Check the same run/asset first; resume a bounded transfer where safe instead of launching another build. |
| Signing or promotion | Stop mutation; retain the exact state and report whether tag, draft, assets or public feed already changed. |

For UI failures, retain page state, worker status/cursor and logs. Increasing a
timeout is justified only by measured progress and the workload's intended lane;
it is not a replacement for selecting an appropriate release check.

Use existing candidate bytes for focused diagnosis when product code is
unchanged. Every required gate must eventually pass against the promoted bytes;
an old or failed candidate never inherits success by assertion. Keep repairs
bounded, retain reproducible results and delegate a factual failure-ledger entry.
Load historical hosted-fix documents only when their specific issue is relevant.
