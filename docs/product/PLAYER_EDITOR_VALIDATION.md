# Player Editor validation

Status: unpublished development feature, validated 2026-10-15 on one owner-run
PC v2.02 character. This adds a top-level Player Editor beside Scroll search and
Equipment & items. Existing equipment, item and currency flows are preserved.

## Native scope

The only admitted executable is PC FILEVERSION `2.0.2.0`, SHA-256
`e22c4a635e4ec1e27a177b76e27d7f6a637f426c0ed3928b60f5693bc52ae130`.
The runtime joins independent player/inventory ownership, checks pinned native
instructions and requires two stable reads and matching process-birth identity.

| Outcome | Native evidence and mutation boundary |
| --- | --- |
| Available prestige | Nine native title-category budgets; edit one existing four-byte earned-budget field to represent the desired available points plus existing allocations. The editing bound is 2000 total; titles, fame and bonus allocations are preserved. |
| Stored ingot Amrita | Owned item `0xDF4D`; stored-Amrita UI at RVA `0x1E3B7C0`, clamp/write leaf `0x217F330`. One eight-byte stored value, maximum 9,999,999,999,999. |
| Soul Fragments | Balance UI `0x1E131D4`, consume path `0x20C926C`, native clamp `0x2154024`. One four-byte usable value, maximum 999,999,999. |
| Spendable Glory | Held-balance UI `0x274FB9C`, native gain `0x220A108` and spend `0x220AC58`. Two four-byte account/player balances, maximum 2,000,000,000; no sequence-ID mutation. |
| Per-clan rank | Existing-record lookup `0x2935ED4`, effective contribution `0x2935F0C`, rank thresholds `0x220D630`. Change one eight-byte contribution value; preserve membership, other clans and war scores. |

Tenure follows the native raw clock/multiply/shift calculation. Unsupported clock
frequency and unrepresentable target ranks are refused. Rank is derived from
contribution, rather than patched as a display label. Fifty clan IDs and ten
thresholds are represented; absent records and unowned ingots are not created.
English/Japanese clan translations are curated UI text, not captured native
locale evidence. Raw executable sections and private captures remain local.

## Protected operations and live correction

All edits require a reviewed snapshot and confirmation. A shared admission lock
and durable receipt precede write access. Operation UUID replay returns the
recorded result; uncertain writes block both prestige and resource edits. Recovery
observes the recorded process without retrying or rolling back a game write.
Amounts cross the UI/IPC boundary as decimal strings. Partial Glory writes are
not atomic and retain uncertainty until their state can be established.

The first live resource read found different account/player synchronization
markers while balances agreed. The owner performed a normal Glory gain/purchase,
after which the markers agreed. Both markers now remain independently in snapshot
identity. Their equality is required specifically for Glory editing; other
resources remain usable. `glory_editable` controls the localized read-only UI.
Neither marker is repaired or fabricated.

## Completed checks

| Layer | Result |
| --- | --- |
| Runtime library | 237 tests passed, including exact patch bytes/neighbors, stale state, PID reuse, native bounds, rank/tenure boundaries and marker preservation. |
| Protected library and contracts | 62 library tests and five contract tests passed; durable receipts, cross-scope blocking, lost responses, partial writes and read-only recovery covered. |
| Desktop and broker | 110 desktop IPC tests passed, one skipped; one Tauri broker test passed for all eight player routes and four recovery/edit jobs. |
| Static/type/localization | Pinned native signatures/catalog coupling, warnings-denied protected Clippy, TypeScript and 1,721 locale entries passed. |
| Scripted UI | 144 assertions passed in Chinese, English and Japanese, light/dark themes and constrained windows. These use production components over a scripted bridge. |
| Live writes and display | Five protected edits/readbacks passed: one available prestige point, 1000 stored Amrita, one Soul Fragment, 100 Glory and one existing clan from rank 5 to 6. The owner confirmed each visible outcome. |
| Native use and reload | The owner performed normal ingot deposit/withdrawal. After owner-reported normal save and same-character title-screen reload, fresh read-only observations retained all five resulting values and other category/clan data. |

Public UI previews use scripted data, not private game captures:
[light](images/player-editor-light.png), [dark](images/player-editor-dark.png).

With the repository build-root environment configured, the source gates are
`cargo test --manifest-path crates/nioh3-runtime/Cargo.toml --lib`,
`cargo test --manifest-path crates/nioh3-protected/Cargo.toml --lib --test player_resources_contract --test prestige_contract`,
`cargo clippy --manifest-path crates/nioh3-protected/Cargo.toml --all-targets -- -D warnings`,
`npm run typecheck`, `node tools/audit_v2_ui_locales.mjs` and
`node apps/workshop/verify-prestige.mjs`. Run desktop IPC tests through the
repository-prepared Python environment. None of these gates authorizes live writes.

## Remaining acceptance

Full game-process restart, online/account refresh persistence, broader resource
spending, protection-effect behavior, native WebView2 and packaged-app acceptance
remain unverified. Same-process title reload is not independent encrypted-save
decoding or proof of server authority. No release version, asset or feed changes
are included.
