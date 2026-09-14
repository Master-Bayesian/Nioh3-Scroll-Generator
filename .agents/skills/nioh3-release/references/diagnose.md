# Diagnose mode

Diagnose the failed stage and preserve its evidence. Do not dispatch, rebuild, publish, or weaken a gate merely to obtain a green result.

## Evidence first

Collect the full candidate SHA, workflow and run IDs, exact failed step, failed logs, artifact names and hashes if any, and the preceding successful gates. For hosted failures, use read-only GitHub queries keyed by exact commit SHA; do not select a run only by branch name or recency.

Classify the failure before editing:

- checkout cleanliness, generated files, line endings, version, or source/native identity;
- dependency installation, locked toolchain, or hosted environment;
- unit, integration, native-fault, WebView2, packaged, or one-file acceptance;
- archive, manifest, filename/URL, download-budget, hash, or signature identity;
- upload, permissions, tag, or publication state.

Read only the matching section of [hosted build fixes](../../../../docs/knowledge/V070_HOSTED_BUILD_FIXES_20260909.md). Preserve the gate that exposed the failure: do not bless a modified hash, force a click, broaden a retry, strip canonical path prefixes, substitute an environment-only WebView2 switch, or accept repeated test IDs.

## Stop and repair

A SHA with a failed release acceptance run is not publishable. Record the run and cause, retain failed binaries only for diagnosis, and never dispatch the same known-bad SHA again. If a repair is requested, make the narrow fix, run focused regression coverage, commit it as a new SHA, and return to prepare mode. If the task asks only for diagnosis, report the cause and required repair without implementing it.

For a UI timeout, require the captured UI state and logs. For a signature failure, distinguish missing or wrong secret material from filename/URL/version/hash metadata rejection. Never replace the production key with a test key or claim authenticity from a SHA-256 digest alone.

End with the exact failed SHA/run, supported root cause, evidence, whether any artifact is non-promotable, and the next gate for a new candidate. Mark uncertainty explicitly when evidence is incomplete.
