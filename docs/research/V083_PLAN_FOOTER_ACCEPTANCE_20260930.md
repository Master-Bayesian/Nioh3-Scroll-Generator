# Character save-plan footer acceptance

Owner report: a five-item plan requires scrolling all details before finding
the title-screen confirmation and commit buttons. The supplied ZIP also records
a test7-r2 stale-plan rejection incorrectly retained as an unknown write. The
report is evidence; its proposed instructions are not adopted as authority.

## Failure cases, before implementation

- Long five-/sixteen-item plans scroll the whole card, including confirmation
  and actions. Only details may scroll; the footer stays inside the visible card.
- A narrow native viewport or wrapped English/Japanese note displaces actions
  below the window. Check wide/narrow layouts and scale equivalents, all locales.
- Scrolling details moves the footer or prevents reaching the last item.
- Confirmation is bypassed, busy controls remain active, or discard sends a
  write. Preserve confirmation, backup/receipt contracts and discard semantics.
- Layout changes mutate item/effect values. Yasakani raw value 1 stays unchanged.
- The host explicitly rejects a changed save with "no write attempted", yet
  the client retains an unknown operation id and queries a nonexistent ledger.
  A confirmed pre-write refusal must invalidate the old plan, refresh the save
  and keep the queued items for a newly prepared plan.
- A lost reply or genuinely unknown result is mistaken for a refusal. Those
  states must remain fenced and retain the existing receipt recovery flow,
  including an error/path which merely embeds the known refusal wording.

## Scope and completion

CharacterEditor save-plan presentation and narrowly classified commit refusal,
with production UI/SaveSession E2E and repeatable native/package artifacts under
`D:/Nioh3_v080_deliverables/deliverables/codex-v083-plan-footer-20260930/`.
No unit tests are added. No real save or game is modified. Game shutdown is
never required; the existing title-screen confirmation remains intact.

## Implementation and bounded evidence

`CharacterEditor.tsx` separates the scrollable plan details from its fixed
confirmation/footer actions. Shared CSS caps the card; the footer does not
participate in detail scrolling. Controls also follow SaveSession busy state.
No item/effect value, seed generation or save-writing rule is changed.

`SaveSession.commit` releases its provisional uncertainty only for the exact
host refusal "Save changed after preparation; no write attempted", optionally
prefixed by the protected error code. `save_app.rs` emits that refusal before
creating durable write intent. The old plan remains invalidated, the UI reloads
the save, and queued items remain available for a new reviewed plan. Other
errors, including a diagnostic which only embeds that wording, retain recovery.

The production UI/SaveSession harness records browser, native and packaged
evidence. Native maximize/restore is real; the 125%/150% constrained checks use
CDP viewport/scale emulation inside WebView2 and do not change system DPI.
All fixtures are presentation/protocol stand-ins; no actual save or game write
is part of this acceptance. The original user feedback is retained privately
under the external task's `input/` directory and is not committed.
