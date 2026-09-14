# Package review

## Evidence selection

For each required Pro output, identify the smallest evidence set that could prove or falsify it. Prefer machine-readable raw captures plus the exact collector and parser over screenshots alone. Include the relevant production source and regression tests, not the entire repository.

Separate:

- confirmed native control flow or byte parity;
- controlled runtime observations;
- interpretations that remain inferred;
- unanswered questions.

State why each positive and negative control isolates the variable. Preserve runtime addresses only as observations and record stable RVAs or signatures separately.

## README reading order

The package README should name:

1. the question and scope boundary;
2. the authoritative starting files in reading order;
3. the strongest confirmed facts;
4. incomplete, stale, or abandoned leads;
5. how to reproduce package-local checks;
6. evidence that is intentionally absent and why.

## Pro task prompt

The prompt should request concrete artifacts such as a derivation, pseudocode, data-layout map, experiment design, implementation contract, and falsification tests. Require the model to cite package-relative evidence paths and to keep facts, observations, inferences, and unknowns separate.

Do not ask the Pro model to assume legality, persistence, version portability, or product safety from a single rendered item, synthetic record, current address, or successful memory write.

## Final boundary check

Structural validation does not establish:

- that copied evidence is authoritative;
- that privacy review is complete;
- that a native interpretation is correct;
- that a generated item is naturally obtainable;
- that a write persists or propagates;
- that a feature is safe to ship.

Name every missing acceptance category in the handoff rather than converting it into a passing claim.
