# Special-rule feasibility and exact empty intersections

## Native structural preflight

The PC v2.00.02 rule generator selects at most three ordered rows. The product
preflight reproduces every deterministic gate that can reject a requested rule
set before Seed search:

- row enabled flag and playthrough-specific nonzero weight;
- duplicate keys and the two native conflict-group references per row;
- the initial integer budget in `1..5`;
- the second-slot opposite-sign rule;
- the third-slot remaining-budget sign and magnitude rule;
- the third-slot maximum-absolute-cost replacement rule;
- the three-slot output limit.

The preflight searches every selectable ordering for every initial budget. A
failure is therefore a deterministic structural impossibility. A pass means
only that at least one abstract native ordering exists; exact Seed replay is
still required because the effect, enemy, and rule generators derive correlated
states from the same displayed Seed.

## Reported R4 empty intersection

The following two rules are structurally compatible:

- `0x2FEA`, cost `-4`: Cursed Cavalcade (Dual Swords);
- `0x7EF1`, cost `+1`: increased priority drop rate (Tsukuyomi's Grace).

They use different enabled conflict identities. One legal structural witness is
initial budget `1` with ordered keys `0x7FB5 -> 0x2FEA -> 0x7EF1`.

However, the complete conjunction below has no Seed:

- rarity 4, playthrough 3;
- primary effect `0x774F`;
- Hattori Hanzo (`0x000202A7` or `0x000D35E1`);
- Onryoki (`0x00040A3B`);
- special rules `0x2FEA` and `0x7EF1`.

The full `2^32` pivot family was exhausted by
`research/verify_reported_rare_combination.py`. Exact cumulative survivors were:

| Stage | Survivors |
|---|---:|
| Primary `0x774F` | 6,177,768 |
| Hattori Hanzo | 163,831 |
| Onryoki | 7,496 |
| Rule `0x2FEA` | 7 |
| Rule `0x7EF1` | 0 |

This is an exact empty intersection, not a static conflict. It remains a
reproducible research vector for the general solver; the product does not carry
an ad-hoc blacklist for this individual request.
