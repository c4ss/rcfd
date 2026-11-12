# ADR 0341: F-439 registration: coarsening on the directional lattice by a box merge

Status: **REGISTERED 2026-11-12**, written before any code of F-439 and before any run of its checks. Not yet accepted by the
maintainer. Results go in the addendum at the end; nothing above the addendum is changed after a run.

Parent: [ADR 0267](0267-f348-registration-anisotropic-refinement-per-axis-levels.md) (F-348: `AnisoMesh`, `AxisLeaf`,
`AxisMark`, the pair merge of D3, the missed G6, amendment 1 with its five-box tiling, G6b, G6c and R2). Roadmap item F-439
(depends on F-348). Number 0341 given by the lead. Branch `f439-aniso-box-merge` from main 916bd91a, worktree `w49i-work/f439`.

House rules (as ADR 0267):

- limits are fixed here, before the code and the runs, and are not changed after a run; a missed limit becomes an ignored
  `*_known_miss` test with the limit intact and the number recorded; a defective set-up gets a corrected test by an amendment
  committed before the corrected run; each registered run is made once;
- defaults stay bit-identical: `AnisoMesh::adapt` keeps its signature and its results (it becomes a call of the new
  `adapt_with` with the pair rule, whose pair code is the existing code, moved, not changed); `AmrMesh`, `Leaf`, `Mark`,
  `Transfer`, `transfer_field` and `src/adapt.rs` are not touched; no existing test or ignored record is edited (the ignored
  `g6_coarsening_returns_to_the_base_known_miss` stays as it is); no hash table and no golden is touched; nothing outside
  `crates/rcfd-mesh/src/adapt/aniso.rs` and one new test file `crates/rcfd-mesh/tests/adapt_aniso_box_f439.rs` changes, apart
  from this ADR and `docs/meshing.md`;
- no new third-party crate, no `unsafe`, the crate graph unchanged, unsigned commits, no trailers, no push.

## What F-439 asks

Acceptance (quoted, binding): "On a lattice with three refined axes, repeated all-axes coarsening returns every state made by
splits and merges, including the five-box tiling of ADR 0267 amendment 1, to the base with sum V q conserved; slab results and
the isotropic AmrMesh paths stay bit for bit."

Reading of the clauses (taken on the maintainer's standing instruction of 2026-11-11, take the recommendation):

- "repeated all-axes coarsening" is the new opt-in rule (`Coarsening::Box`) with `AxisMark::coarsen([true; 3])` on every cell,
  balance off (as the registered G6 of ADR 0267; with balance the return is not derived, see D4, and is recorded only);
- "every state made by splits and merges": any state reached from the base by refinement rounds (balance on or off), pair
  coarsening rounds and box coarsening rounds with any marks. D3 proves the return for every tiling by dyadic boxes, which
  contains all of them; the checks sample them;
- "slab results and the isotropic AmrMesh paths stay bit for bit": everything that does not ask for the box rule (the existing
  `adapt`, G6b in a slab, every `AmrMesh` path) gives exactly what it gave before.

## What was read and computed before this text (disclosed)

- **Read:** the roadmap entries F-439 and F-348; ADR 0267 in full (D1 to D7, the checks, amendments 1 and 2, addendum 1, R2 and
  R4); `crates/rcfd-mesh/src/adapt/aniso.rs` in full; `crates/rcfd-mesh/tests/adapt_aniso_f348.rs` in full; the users of
  `AnisoMesh` (`crates/rcfd-fv/src/adaptive.rs` and its test `adapt_directional_f348.rs` call `adapt` only).
- **No run** of any code was made for this ADR.
- **Arithmetic (disclosed, by hand):** the outcomes of B1, B3, B4 and B5 below were worked out on paper from the rule of D2
  (the leaf order, the candidate boxes of each leaf in the registered order, which of them are exactly tiled). They are written
  in the table as the limits.

## Design (registered)

### D1. The rule and its API

`pub enum Coarsening { Pair, Box }` and `AnisoMesh::adapt_with(&self, marks, balance, coarsening) -> Result<(AnisoMesh,
Transfer, AdaptReport)>`. `adapt(marks, balance)` is `adapt_with(marks, balance, Coarsening::Pair)`. The refinement part (the
request queue, the ripple of the balance, `ignored_at_max_level`) and the construction of the `Transfer` are shared and are the
existing code; only the coarsening loop has a second branch. `AdaptReport::coarsened` counts merges (one per merged box) in both
rules.

### D2. The box merge

Notation (ADR 0267 D1): a dyadic box of the lattice is a key `(levels, ijk)`; its **ancestor by `d`** (`d` a vector of
non-negative integers with `d_t <= levels_t`, `d_t = 0` on an unrefined axis) is `(levels - d, ijk >> d)` (per axis), the
unique dyadic box with those levels that contains it. A box `A` is **exactly tiled** in a leaf set when every leaf of the set
that meets `A` lies inside `A`; the leaves inside then tile `A` (the set tiles the domain).

For each old leaf `l` in cell order with a mark that has no `refine` axis, that is still a leaf of the current set `cur` (not
split by the balance ripple and not taken by an earlier merge of this call): let `C(l)` be the refined axes `t` with
`coarsen[t]` and `l.levels[t] > 0`. The candidates are the ancestors of `l` by every `d != 0` with `d_t = 0` off `C(l)` and
`1 <= d_t <= l.levels[t]` or `d_t = 0` on `C(l)`, in the order: smaller `sum_t d_t` first (smaller box first), then `d`
lexicographically larger first (so among single-axis candidates x, y, z, the order of the pair rule). The first candidate `A`
that passes all of the following is merged:

1. `A` is exactly tiled in `cur` (D1's `overlapping(cur, lo(A), hi(A))` returns only leaves inside `A`);
2. every leaf `n` inside `A` is a leaf of the old mesh (not made in this call) whose mark has no `refine` axis and has
   `coarsen[t]` on every axis `t` with `n.levels[t] > A.levels[t]` (**consent**: no leaf is merged along an axis it did not ask
   for; `l` consents by the choice of `d`);
3. with `balance`, the merged leaf `A` differs by at most one level along every refined axis from every face neighbour in `cur`
   after the merge (the test of the pair rule, `balanced`).

The leaves inside `A` are removed from `cur` and `A` is inserted; `coarsened += 1`. Leaves made in this call are never inside
a later merge (rule 2), so each old leaf takes part in at most one merge per call, as in the pair rule.

**A merge of two buddies is the pair merge.** The ancestor by `e_t` of `l` is `l`'s parent along `t`; if it is exactly tiled by
two leaves they are `l` and its buddy along `t`. With single-axis marks (`C(l) = {t}`, levels 1 along `t`) the candidates are
that one box, and the box rule makes the pair rule's merges (check B3). In general the box rule merges more: a box exactly
tiled by more than two leaves, or by two leaves of different levels (for example a half split along y beside an unsplit half),
which the pair rule never merges.

**Result levels.** The merged leaf has the levels of `A`: `l.levels - d`; along an axis where the leaves inside had different
levels the result has `A`'s level, which is at most the smallest of theirs.

### D3. Why repeated all-axes box coarsening returns any state to the base (derivation, first principles)

Let `T` be any tiling of the domain by dyadic boxes of the lattice (every state made by splits and merges of either rule is
one: a split replaces a box by a tiling of it, a merge replaces a tiling of a box by the box). Every leaf lies inside one base
cell (a dyadic box with levels `>= 0` is a sub-box of the base cell `ijk >> levels`), so every base cell is exactly tiled.

Take all-axes coarsen marks, balance off. Let `c` be a base cell that is not a single leaf, and `l` the first leaf of `c` in
cell order. When `l` is processed, either a leaf of `c` has already been merged in this call, or none has: then `cur` agrees
with the old set on `c`, `C(l)` is the set of axes with `l.levels[t] > 0` (non-empty, since `l` is not the whole of `c`), the
ancestor of `l` by `d = l.levels` is `c` itself and is a candidate, and it passes 1 (base cells are exactly tiled), 2 (all
leaves of `c` are old, unmerged and marked along all axes) and 3 (no balance). So the first passing candidate exists and a merge
is made. Hence every call on a state other than the base makes at least one merge, the cell count drops by at least one per
call, and the base (the only tiling with `n_base` cells, all at level 0) is reached within `n - n_base` calls, `n` the count at
the start. No history is needed, so this holds for the five-box tiling and for any other state; refinement can follow from any
of them (ADR 0267 D1: point location, faces and splits do not use a history).

The bound `n - n_base` is the registered limit; the expected number is much smaller: the smallest-first order undoes about one
split level per axis per call, so the expected count is near the largest `sum_t l_t` of a leaf (recorded, not counted).

### D4. The balance rule

With `balance`, rule 3 refuses a merge whose result would differ by two or more levels along some refined axis from a face
neighbour; the next candidate is then tried (a larger box can pass where a smaller fails, when it swallows the fine neighbour).
If the state satisfied rule (B) of ADR 0267 D3 before the call, it satisfies it after the coarsening: every face that changes
has a merged leaf on one side and was checked when that leaf was inserted (a later merge next to it checks the same face again
from its own side). The refinement part of the call is the existing ripple. The argument of D3 does not carry over: the base
cell candidate can be refused by a fine neighbour in another base cell, and I have no derivation that some candidate always
passes. So with balance the return to the base is **recorded, not counted** (B5b), and listed in the limits.

### D5. Transfer

The existing construction of `adapt`, unchanged: a new leaf that is not an old leaf and is not inside one old leaf is
`Source::Merged` of the old leaves over its box (`overlapping` on the old set, sorted). For a box merge these are exactly the
leaves inside `A` (rules 1 and 2: they are old leaves and tile `A`). `transfer_field` gives the merged cell the volume-weighted
mean, so `V_A q_A = sum_i V_i q_i` up to round-off (and `V_A = sum V_i` up to round-off, the volumes being computed by
`Mesh` from the same lattice points): `sum V q` is conserved. For a linear field the volume-weighted mean over a tiling of a box
is the value at the box centroid (the centroid of a box is the volume-weighted mean of the centroids of any tiling of it, ADR
0267 D4), so a linear field stays exact under merges too, for both `Injection` and `Linear`.

### D6. Cost

Per processed leaf at most `prod_t (l_t + 1) - 1` candidates (at most 1330 with `max_level` 10 on three axes), each one
`overlapping` query; a candidate that meets a leaf outside it is rejected at the first such leaf found. The tests use levels
up to 3.

## Registered checks

All in the new file `crates/rcfd-mesh/tests/adapt_aniso_box_f439.rs`, ordinary tests (each well under a second in release by the
sizes involved). `check_topology` is a copy of ADR 0267's (a test file cannot import another). Lattices: (a) the graded 3-D box
3 x 2 x 2 of ADR 0267, `max_level = [2, 2, 2]`, 12 base cells, volume 0.8; (d) a 2 x 2 x 1 unit box (z nodes 0, 1),
`max_level = [3, 3, 3]`, 4 base cells, volume 1; (b) the slab 5 x 4 of ADR 0267, `[3, 3, 0]`.

| Id | Check | Limit | Estimate behind the limit |
| --- | --- | --- | --- |
| B1 (acceptance: "including the five-box tiling of ADR 0267 amendment 1") | G6c's construction (one cell `[0, 1]^3`, `[1, 1, 1]`, octants, the three pair merges) gives the five-box tiling; then one call of `adapt_with(all-axes coarsen, balance off, Box)` | 5 cells before; after: 1 cell, levels `[0, 0, 0]`, `coarsened = 1`, the transfer `Merged([0, 1, 2, 3, 4])`; a random field's `sum V q` within `1e-13` relative; `check_topology` | exact (D2 by hand: for the first leaf, the octant at the origin, the six candidates of sums 1 and 2 each meet one of the three half-slabs outside them; the cell is the first that passes) |
| B2 (acceptance: "returns every state made by splits and merges ... to the base with sum V q conserved") | states: (i) G6's state (lattice (a), G6's seed and three refinement rounds, 146 cells); (ii) the state where G6's pair rounds stop (16 cells, the stuck tiling); (iii) 12 random sequences on (a) and 12 on (d) (seeds 1 to 12 and 101 to 112): 5 rounds, the first a refinement round, each later one by the generator a refinement round (per cell probability 0.4, a random non-empty axis set), a pair coarsening round or a box coarsening round (per cell probability 0.6, a random non-empty axis set), balance on or off by the generator. From each state: all-axes box coarsening rounds, balance off, a random field and the linear field `1.3 + 0.7 x - 0.4 y + 0.2 z` moved by `transfer_field` (`Linear`) | the base reached (`leaves() ==` the base's) within `n - n_base` rounds; the count strictly decreases each round until then; `check_topology` every round; `sum V q` of the random field within `1e-13` relative of `sum V |q|`; the linear field equal to the closed form at the new centres within `1e-12` of its range every round; at least one of the 26 states is one where pair all-axes coarsening (balance off) stops before the base (state (ii) at least) | exact (D3); round-off (D5) |
| B3 (single-axis marks give the pair merges) | octants of `[0, 1]^3` with G6c's three single-axis marks, `Box` against `Pair` (balance off and on) | the five-box tiling from `Box` (5 cells, 3 merges); the leaves, the `Transfer` and the report of the two rules equal | exact (D2: the only candidate of a level-1 leaf with one marked axis is its parent along that axis) |
| B4 (consent and marks) | cell `[0, 1]^3`, `[1, 1, 1]` (in a 1 x 1 x 1 base, the 2-D argument applies in the xy plane): split along x, then the left half along y (3 leaves L1, L2 at levels `(1, 1, 0)`, R at `(1, 0, 0)`). (a) L1, L2, R all-axes: 1 call gives 2 cells (L1, L2 merged into the left half), a second call 1 cell; (b) L1, L2 coarsen x only, R all-axes: 3 cells (no merge); (c) from the two halves, left all-axes, right coarsen y only: 2 cells (no merge); (d) the five-box tiling (built as in B1 but with `max_level = [2, 2, 2]`) with the octant at the origin `KEEP` and the others all-axes: 5 cells; (e) the same with that octant marked `refine([true; 3])`: that octant split into 8, no merge, 12 cells | the counts and `coarsened` as stated | exact (by hand, D2 rules 1 and 2) |
| B5 (balance refuses a merge) | `refine = [true, false, false]`, x nodes 0, 1, 2, `max_level = [2, 0, 0]`: both base cells split along x, then the two halves of cell 1 split again (leaves `[0,2], [2,4]` at level 1, `[4,5] .. [7,8]` at level 2); marks: the two level-1 leaves all-axes coarsen, the others `KEEP`; `Box` with balance on and off | balance on: 6 cells, `coarsened = 0`; balance off: 5 cells, `coarsened = 1`, the merged leaf at level 0; both meshes pass `check_topology` | exact: the only candidate of `[0, 2]` is the base cell `[0, 4]`, at level 0 beside `[4, 5]` at level 2 |
| B5b (balance kept) | lattice (a) refined by G1's balanced random sequence (seed 12345, 4 rounds of G1's marks with balance on), then all-axes box rounds with balance on until a round makes no merge (at most 50) | every round: `max_axis_level_jump <= 1` on every axis, `check_topology`, `sum V q` within `1e-13`; whether the base is reached and the number of rounds recorded, **not counted** | D4 |
| B6 (acceptance: "slab results and the isotropic AmrMesh paths stay bit for bit") | (a) `adapt(m, b)` against `adapt_with(m, b, Pair)` on G1's three lattices and seeds (balance on and off, 4 or 5 rounds): leaves, `Transfer`, report and `to_mesh` (points `to_bits`, faces, owners, neighbours, patches) equal; (b) `crates/rcfd-mesh/tests/adapt_aniso_f348.rs` (G6b the slab result among them) and `crates/rcfd-mesh/tests/adapt.rs` (the isotropic `AmrMesh` tests) run unedited; the ignored record `g6_..._known_miss` is run once with `--ignored` to show it still records the same miss; `rcfd-fv` `adapt_directional_f348` (its non-ignored tests, which call `adapt`) run unedited; (c) `git diff main` shows no change outside `aniso.rs`, the new test file and the docs, and no line of the pair branch's code changed | all equal; all pass; the known miss fails with the recorded "round 6: no merge" (146 cells, 16 left); diff as stated | the pair branch is the old code, moved |

Recorded, not counted: the rounds to the base of every B2 state and their cell counts per round; how many of the 24 random
states pair coarsening leaves stuck and with how many cells; B5b's outcome; times.

### Negative controls

B2(ii) is the state on which the pair rule stops (G6's known miss); if box coarsening returned it to the base only because the
state is not stuck, B1 and the recorded pair-stuck count of B2 say so. B4 (b) to (e) and B5 (balance on) are states where the
box rule must not merge; they guard against a rule that merges too much.

### Mutations (scratch copy, not committed)

Each alone on a `git archive` copy of the final commit in `w49i-work/f439-scratch`, by an exact text replacement (each pattern
found once), the tests of the new file and of `adapt_aniso_f348.rs` run once in release in their own target directory.

| Id | Mutation | Must |
| --- | --- | --- |
| MB1 | rule 1 dropped: a candidate is accepted when the leaves found inside are fine, leaves meeting it from outside ignored | B1 fails (overlapping leaves or a wrong count) |
| MB2 | rule 2's consent dropped (marks of the other leaves inside ignored) | B4 fails |
| MB3 | candidates in the order larger box first | B3 fails |
| MB4 | `d_t` limited to `l.levels[t] - 1` (the base-cell candidate never offered for a level-1 axis) | B1 fails |
| MB5 | rule 3 skipped in the box branch | B5 fails |
| MB6 | `adapt` routed to the box rule | B6 (a) and ADR 0267's G6c fail |

### Predictions (mine)

| Id | Prediction |
| --- | --- |
| P1 | B1 to B6 met at the first run |
| P2 | every B2 state reaches the base in at most 7 rounds (state (i) in 4 to 6) |
| P3 | B5b reaches the base |
| P4 | at least a third of the 24 random states are pair-stuck |

## Decision rule (registered)

- **done**: B1, B2, B3, B4, B5, B6 met, B5b's counted part (balance kept, topology, conservation) met.
- **partial**: B1, B2 and B6 met but a supporting check (B3, B4, B5, B5b's counted part) missed, each miss a `*_known_miss`
  with its number and a diagnosis.
- **todo**: B1, B2 or B6 missed (the acceptance is not met, or a default moved).

## Limits of this item

- The return to the base is derived and checked with balance off; with balance it is recorded only (D4).
- The rule is not used by the adaptive loop of `rcfd_fv::adaptive` (which does not coarsen); coarsening in the loop is ADR 0267
  R4 (b), a later item.
- Box lattices only, as ADR 0267. The box rule is not the isotropic lattice's coarsening on isotropic states (`AmrMesh` merges
  `2^d` siblings in one step; the box rule's smallest-first order merges pairs first); it is not offered as such.
- Candidate enumeration costs up to `prod (l_t + 1)` overlap queries per leaf (D6); no index of exactly tiled boxes is kept.

## Open questions and choices for the maintainer

- **Q1. API.** Options: (a) an enum `Coarsening` and `adapt_with`, `adapt` unchanged; (b) a separate `adapt_box`; (c) make the
  box rule the default of `adapt` (changes F-348 results). Taken on the maintainer's standing instruction of 2026-11-11 (take
  the recommendation): (a).
- **Q2. Candidate order.** Options: (a) smallest box first (undoes about one level per axis per call; equals the pair rule on
  buddy pairs); (b) the base cell first (back to the base in one call; no gradual coarsening). Taken on the same standing
  instruction: (a).
- **Q3. Consent.** Options: (a) every leaf inside must be marked along every axis on which it is coarsened (D2 rule 2); (b) the
  mark of the leaf that starts the merge suffices. Taken on the same standing instruction: (a), which is the pair rule's "the
  buddy agrees" generalised.

## Addendum 1: results (2026-11-12)

Commits: 90bff8eb (this registration alone), 0e96b706 (the code and the tests). Runs, all in release, target directory in
`w49i-work/f439-scratch`:

- B checks: `cargo test --release -p rcfd-mesh --test adapt_aniso_box_f439 -- --nocapture --test-threads=1`, once (log
  `f439-scratch/run_b.log`), on the code that became 0e96b706; the commit differs from the run state by `cargo fmt` only (line
  breaks in one expression of `box_merges` and in two test helpers).
- Existing tests, unedited: `rcfd-mesh` `adapt_aniso_f348` (9 passed, 1 ignored) and `adapt` (12 passed); the ignored record
  `g6_coarsening_returns_to_the_base_known_miss` run once with `--ignored`: it fails as recorded, "round 6: no merge"; `rcfd-fv`
  `adapt_directional_f348` (5 passed, 7 heavy ignored, not run).
- No limit was changed; no check missed; no amendment.

### Results

| Id | Limit | Measured | Met |
| --- | --- | --- | --- |
| B1 (acceptance: the five-box tiling) | 5 cells to 1 at level 0 in one call, `coarsened = 1`, `Merged([0..4])`, `sum V q` to `1e-13`, topology | as registered; integral 0.5815023819026417 to 0.5815023819026416 | yes |
| B2 (acceptance: every state to the base, `sum V q` conserved) | base within `n - n_base` rounds, strict decrease, topology, `sum V q` `1e-13`, linear field `1e-12` of range, at least one pair-stuck state | all 26 states back to the base: (i) G6's 146 cells in 5 rounds (60, 29, 16, 14, 12); (ii) the pair-stuck 16 cells in 1 round; (iii) 24 random states (6 to 182 cells) in 1 to 6 rounds; 6 of the 26 states are pair-stuck ((ii) and 5 random ones, stuck at 18, 20, 18, 18 and 8 cells) | yes |
| B3 | single-axis marks: `Box` equals `Pair` (leaves, `Transfer`, report), the five-box tiling, balance on and off | equal | yes |
| B4 | (a) 2 then 1 cells; (b) 3; (c) 2; (d) 5; (e) 12 with `refined = 1` | as registered | yes |
| B5 | balance on: 6 cells, no merge; off: 5 cells, the merged leaf at level 0 | as registered | yes |
| B5b (counted part) | jump `<= 1`, topology, `sum V q` `1e-13` every round | met in every round | yes |
| B5b (recorded) | | 226, 120, 67, 32, 14, 12: the base reached with balance in 5 rounds | recorded |
| B6 (acceptance: slab results and the isotropic paths bit for bit) | (a) `adapt` = `adapt_with(.., Pair)` on G1's sequences; (b) the existing tests pass unedited, the known miss unchanged; (c) diff only in `aniso.rs`, the new test file and docs, no pair-code line changed | (a) equal in all 26 rounds; (b) as listed above; (c) `git diff main --stat`: `aniso.rs` (135 added, 35 removed), the new test file, this ADR, `docs/meshing.md`; the 35 removed lines are the transfer construction at the end of `adapt`, re-added unchanged after the coarsening `match`; the pair loop is the old text, now the body of `pair_merges` (it takes and returns the leaf set by value, so not one of its lines changed) | yes |

Recorded: times 0.20 s for the whole file. Against the predictions: P1 met; P2 met (at most 6 rounds; (i) 5); P3 met (B5b reached
the base); P4 **not met** (5 of 24 random states pair-stuck, not 8 or more).

### Mutations (scratch copy `f439-scratch/mut`, a `git archive` of 0e96b706, target `f439-scratch/target-mut`, release)

Each alone by exact replacement (each pattern found once, `f439-scratch/mutate.py`, logs `f439-scratch/mut_MB*.log`), the tests
of `adapt_aniso_box_f439` and `adapt_aniso_f348` run once, the file restored and compared afterwards (identical).

| Id | Mutation | Failed tests | Caught by the registered test |
| --- | --- | --- | --- |
| MB1 | exact-tiling test dropped | B1, B2, B4, B5b | yes (B1) |
| MB2 | consent dropped | B4 | yes |
| MB3 | larger box first | B4 | **no**, caught by B4 instead of B3 |
| MB4 | `d_t <= l_t - 1` | B1, B2, B3, B4, B5 | yes (B1) |
| MB5 | balance test skipped in the box branch | B5, B5b | yes (B5) |
| MB6 | `adapt` routed to the box rule | B2, G6b, G6c | yes (G6c); B6 (a) passed |

Every mutation is caught. Two gaps in my reasoning, not in the coverage: in B3 every leaf has one candidate (a level-1 leaf
marked along one axis), so the candidate order cannot matter there; B4 (a) catches the order (largest first merges the three
leaves into the cell in the first call). In B6 (a) G1's random marks coarsen along one axis with probability 0.15, and on
those states the two rules happen to make the same merges; G6b, G6c and B2 catch the routing.

### Format and lint

`cargo fmt -p rcfd-mesh --check` clean after `cargo fmt` (applied after the registered run, line breaks only);
`cargo clippy --release -p rcfd-mesh --all-targets -- -D warnings` clean.

### Deviations and what was not run

- D6 says a candidate is rejected at the first leaf found outside it; the code collects the overlap list of the candidate and
  then tests it (same result, somewhat more work per rejected candidate).
- MB3 and MB6 were caught by other tests than the ones the registration named (above).
- Not run: the whole workspace suite (by instruction); the heavy tests of `adapt_directional_f348` (they call `adapt`, whose
  results B6 shows unchanged); `rcfd-api` tests (`rcfd-api` does not use `AnisoMesh`, and no shared solver file was touched).

### Decision by the registered rule

**done.** B1, B2 and B6 (the three clauses of the acceptance) and the supporting checks B3, B4, B5 and B5b's counted part are
met at the first run; nothing missed. The rule does not ask for the predictions; P4 missed and changes nothing.

### Remaining questions

- **R1. F-348's status.** ADR 0267 R1 recommended partial "until R2 is settled"; R2 is settled here for the box rule. Options:
  (a) leave F-348 partial (its G6 and C5 stay missed registered checks by its own rule); (b) mark it done now that its open
  coarsening point is delivered. Recommendation (a): its rule gave partial and only the maintainer can override it; its note
  gets one sentence (below).
- **R2. Return to the base with balance.** Not derived (D4); one run reached it. Options: (a) leave it recorded; (b) a
  derivation or a counterexample as a follow-up. Recommendation (a) until the loop coarsens with balance (R3).
- **R3. Coarsening in the directional loop** (ADR 0267 R4 (b)): the box rule makes it possible in 3-D. Options: (a) a
  follow-up item; (b) leave it. Recommendation (a), item below; it adds a coarsening criterion, so a new parameter, opt-in.

### For the lead: roadmap entries

Proposed status of F-439: **done**. New note:

> DONE (ADR 0341, 2026-11-12): rcfd_mesh::adapt::aniso::AnisoMesh::adapt_with(marks, balance, Coarsening::Box), opt-in: for each cell marked coarsen along axes C, the smallest dyadic ancestor box (levels lowered only along C) that is exactly tiled by leaves of the mesh, each marked along every axis on which it is coarsened, is merged into one leaf with the levels of the box; with balance the merge is accepted only if the new leaf stays within one level of its face neighbours along every refined axis; the transfer is the existing Merged source (volume-weighted mean, so sum V q is conserved and linear fields stay exact). Under all-axes marks the base cell is always an admissible ancestor, so every call merges at least once and repeated all-axes coarsening without balance returns any tiling to the base. The five-box tiling of ADR 0267 amendment 1 goes to one cell in one call; 26 states on two 3-D lattices (G6's 146 cells, the pair-stuck state, 24 random sequences of splits, pair merges and box merges, 6 of them pair-stuck) return to the base in at most 6 calls with sum V q to 1e-13 and linear fields to 1e-12; single-axis marks give the pair merges; consent and balance refuse the registered cases. adapt (Coarsening::Pair) is unchanged: equal to adapt_with(.., Pair) bit for bit, the ADR 0267 tests (G6b in the slab among them, the G6 known miss unchanged) and the AmrMesh tests pass unedited. Six mutations caught. With balance the return to the base is not derived (one recorded run reached it).

Note of F-348 (one sentence added at the end; the rest unchanged):

> PARTIAL (ADR 0267, 2026-11-12): rcfd_mesh::adapt::aniso::AnisoMesh, a second lattice with one refinement level per axis (AxisLeaf, AxisMark; AmrMesh untouched): faces are the intersections of the sides of two leaves, with the corners of all face rectangles in the vertex set, so cells are closed for any tiling by dyadic boxes; optional balance of one level per refined axis across every face (at most 2 faces per side in a slab, 4 in 3-D); the existing Transfer and transfer_field. Random directional marks on a graded 3-D box, a slab and a box refined along two of three axes give valid closed meshes (validate, closure and volume to 1e-13) and linear fields transfer to 4.4e-16; on isotropic marks the mesh, the transfer and the adaptive loop equal AmrMesh's bit for bit. Directional criterion in rcfd_fv::adaptive (axis_variations, directional_axes with theta = 1/2, directional_target_marks, run_adaptive_simple_directional and _target). On the ADR 0247 suction layer the directional run has error 0.00298 with 1547 cells against the isotropic 0.01159 with 3841 (limit: at most 1920 cells); held at the isotropic finest size it reaches 0.01158 with 960 cells; with the axis choice inverted 1.18. ADR 0054, 0242 and 0247 tests pass unedited (the heavy ones included); ten mutations each failed their tests. Partial by the rule because two registered supporting checks missed and were corrected under amendments: pair coarsening in 3-D can leave a five-box tiling of a cell that it cannot merge further (valid mesh; slabs return to the base), and the fast layer test on an 8 x 8 base (cell Peclet 10) had no converged solution on its first directional mesh (replaced by a 16 x 16 base). Open: a box merge for 3-D coarsening (R2), the case key (R5), cells spent along x in early cycles (R4). Follow-ups: F-438 (case key), F-439 (three-dimensional box merge). The box merge is delivered by F-439 (ADR 0341, 2026-11-12): repeated all-axes box coarsening returns any state, the five-box tiling included, to the base.

Follow-up item: title "Coarsening in the directional adaptive loop", deps F-439, acceptance:

> On the ADR 0267 suction layer, a directional loop that also box-coarsens cells whose per-axis variation is below a registered fraction of the largest reaches the error E(D) = 0.00298 of ADR 0267's run D with fewer than its 1547 cells and fewer leaves split along x than its 704; runs that do not ask for coarsening are bit for bit unchanged.

> TODO (from ADR 0267 R4 (b) and ADR 0341 R3): the box rule of ADR 0341 makes directional coarsening possible in 3-D; needs a coarsening criterion per axis (a new opt-in parameter) and the loop's coarsening step; with balance the return to the base is not derived (ADR 0341 D4).
