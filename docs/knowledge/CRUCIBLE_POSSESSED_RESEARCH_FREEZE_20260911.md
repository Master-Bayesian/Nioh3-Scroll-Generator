# PC v2.01 Crucible possessed-enemy research freeze

## Decision

The possessed-enemy inverse solver is frozen. Seed `86872488` is not a valid
single-player positive control. The possessed Nuppeppo previously observed for
this scroll appeared only in an online session; the same scroll did not show a
possessed enemy in the controlled single-player run on 2026-09-11.

No product filter, forward oracle, or inverse-search contract may treat this
seed as proof that possession is determined by the scroll seed alone.

## Preserved evidence

Two independent single-player mission-entry captures for seed `86872488`
produced the same mission lookup, placement value, candidate order, assigned
indices, and requested MT cursor plus 624-word state. The candidate vector
contained Nuppeppo occurrences at spawn IDs `0xF3D` and `0xF40`, but the
captured selection mask and `record+0xEA` flags were zero at the observed
selector completion point.

The older selector collector copied `0x1388` bytes around the MT context. Its
first `0x9C4` bytes, which contain the requested cursor and 624 32-bit state
words, match between repeats. Differences after that prefix are retained as
raw evidence and are not evidence of RNG drift.

All capture scripts were read-only. The final observer was stopped and Cheat
Engine reported no remaining breakpoints. The game and save were not modified
by this research pass.

## Current interpretation

It is confirmed that seed `86872488` reproduces the ordinary mission and enemy
candidate structure. It is observed that an online session can add a possessed
enemy that is absent from the controlled single-player run. The additional
input may be session, network, mission-instance, player, or another late-stage
state. Its source and deciding function are unknown.

## Requirements before resuming

Resume only after obtaining a known single-player scroll that visibly contains
a possessed enemy. Capture that seed twice offline, then compare the same seed
offline and online while recording all inputs to the late mask/application
path. Do not start inverse work until at least one offline positive and one
negative control identify the actual final-state write and its upstream input.
