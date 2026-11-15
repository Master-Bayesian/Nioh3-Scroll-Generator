# Quality controls

Use named states and exhaustive transitions for ownership, validation, retry, fallback and terminal outcomes. Keep the guard and its invariant visible in the implementation and the affected feature contract.

- npm run lint uses typed promise rules and exhaustive finite union switches; a default arm cannot hide an omitted finite state. A deliberately partial strict test double has one documented local exception and still rejects all unexpected commands.
- npm run check:quality exercises the production lint configuration with valid, faulty and disabled-rule controls. Parser failures do not count as detecting the intended rule.
- tools/check_models.ps1 runs normal models and adjacent named negative controls. A deliberate fault passes its control only with TLC invariant exit 12 and the exact expected invariant error. Configurations and logs are retained.
- spec/seed_search/README.md maps backend selection, strict CPU policy, bounded dispatch, exact cursor publication, cancellation and continuation to the production worker. It names the two new counterexamples.
- tools/verify_gpu_search_e2e.py drives actual native backends on current PC v2.02 resources and retains seed/cursor/count parity. tools/verify_gpu_worker_e2e.py exercises framed packaged-worker requests; native packaged application gates supply the separate UI and delivery evidence.

The quality runners and strict lint rule were reused from the owner's uncommitted development worktree after source inspection. Optimizer features and their unrelated models remain in that worktree. These finite models and offline comparisons do not prove arbitrary game writes, game legality or all hardware drivers.
