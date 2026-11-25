# ADR 0796: F-886 registration: three formulas recorded from memory, read against their open sources

Status: **REGISTERED 2026-11-25, written before any source was opened, before any change of a record and before any run of
the checks.** Before this text no file of the repository was changed and no comparison was computed. What was done: reading
(below), searches of the tree, and a build (no run) of the test `docs_site`, which is run on this text before its commit for
its links, as the brief asks. Results go in addenda at the end; nothing above them is changed afterwards.

It answers the brief `docs/adr/briefs/f886-sources-read.md` ([the file](briefs/f886-sources-read.md)) and follows
[docs/process/agent-rules.md](../process/agent-rules.md), version 5. Branch `f886-sources-read` from main 25311db5, scratch
`w49i-work/f886-sources-read-scratch`. Model: Claude Opus 5.5.

## What is asked

The acceptance of F-886 in `docs/roadmap.json`: "Every limit not stated here is registered in the ADR before any change. For
each of the three records, the open source named here is read and cited with its locator in an addendum of the ADR (ADRs stay
append-only) and in the item's reference-data fragment: Folden and Aschmoneit (2023, CC BY 4.0), eqs. 43 and 45, for the
Schnerr-Sauer model and the Rayleigh growth rate of ADR 0056 (reference data: 'unverified'); Lienhard, A Heat Transfer
Textbook, eqs. 3.17, 3.20 and 3.21, for the effectiveness and NTU relations of ADR 0039 (reference data: 'formula from memory
... the book was not accessed'); Celik et al. (2008), Eq. 7, for the GCI of ADR 0762 ('Citation from memory'). Each addendum
states, term by term, whether the implemented form agrees with the source, and records a difference as a finding, not a fix.
The superseded 'unverified' or 'from memory' marks are struck through with the source cited, not deleted. No code, default or
test changes (baseline hashes unchanged)."

## What was read

`AGENTS.md`, `docs/process/agent-rules.md` (version 5) and the brief, in full, before anything else; the item F-886 (by a
script); [ADR 0056](0056-wave29b-mixture-cavitation.md) (whole); of [ADR 0039](0039-wave23a-mhd-heat-exchanger.md) the
sections D2, D3, the checks H and the status; of
[ADR 0762](0762-f870-registration-fan01-design-point-state-band-and-ranked-gap.md) the band, the checks and the last
sections, and every line that names the GCI; of [ADR 0086](0086-wave37c-fan01-solution-verification.md) part G; of
`0000-base.part` the sections and rows named below, the fragment `0762-F-870.part`, the README of the fragments; the three
concept pages and the three source pages of the wiki that the brief names; the parts named below of the code;
[lead-process.md](../process/lead-process.md) on the "Sources read" addendum, the
[tolerance table](../process/tolerances.md), [citation-batches.md](../process/citation-batches.md); of
`scripts/process_report.py` sections e and r and the generated pages; `crates/rcfd-cli/tests/docs_site.rs` in outline.

The wiki pages quote forms and worked values of the three sources. They are not a source, and it is said here because two
sample points below (L4 and G7) are known through them: these are the sources' own worked examples and are not unseen. All
other points were chosen here without knowing a result.

## The records, the code and the sources

| Record | Lines that state the formula | Code that implements it | Mark to strike | Source |
|---|---|---|---|---|
| ADR 0056, the Schnerr-Sauer model and the Rayleigh growth rate | ADR 0056, decision D; `docs/reference-data.d/0000-base.part` lines 1231 and 1232 | `crates/rcfd-fv/src/cavitation.rs`: `radius` (line 35), `alpha_of_radius` (40), `radius_rate` (46), `rate` (60) | status cell "unverified" of both rows | T. S. Folden and F. J. Aschmoneit, Physics of Fluids 35, 081301 (2023), arXiv:2211.16323; Eqs. 43 and 45 (acceptance); 5, 6, 44, 46 and 47 in addition, as far as they are there |
| ADR 0039, effectiveness and NTU | ADR 0039, D2; `0000-base.part` lines 567 (ADR 0012, `Q = eps C_min ...`) and 961 | `crates/rcfd-fv/src/hx.rs`: `effectiveness` (line 50), the branches `Counterflow`, `Parallel` and `c == 0`; `solve_exchanger` (190) | 567: "formula from memory of the standard definition"; 961: "citation UNVERIFIED (memory)" | J. H. Lienhard IV and V, A Heat Transfer Textbook, 6th edition, ahtt.mit.edu; Eqs. 3.17, 3.20, 3.21 (acceptance); 3.16, 3.18 and what is stated of `Cr = 0` and `Cr = 1` in addition, as far as they are there |
| ADR 0762, the mesh-convergence estimate | ADR 0762, "The mesh-convergence estimate, `U_num`"; `0762-F-870.part` line 17 | `crates/rcfd-api/tests/f870_fan01_design_point.rs`: `FS` (line 43), `mesh_convergence_estimate` (200); the same form in `fan01_gci_w37c.rs` `observed_order`, `gci` (224, 242) | "from memory as in ADR 0086, no source could be read", as far as Celik et al. covers it | I. B. Celik, U. Ghia, P. J. Roache, C. J. Freitas (and H. Coleman, P. E. Raad in the journal), "Procedure for Estimation and Reporting of Uncertainty Due to Discretization in CFD Applications", J. Fluids Eng. 130, 078001 (2008); Eq. 7 (acceptance); Eqs. 1 to 6 in addition |

"The three rows" of the brief are read as the rows of the three records, which are the five lines above (taken on the
maintainer's standing instruction of 2026-11-11 (take the recommendation)). Of a mark only the words that the reading
supersedes are struck: "the book was not accessed" of line 567 speaks of Kays and London and stays; the rows on the
one-stream-mixed crossflow, shell and tube, the vapour pressure, Manninen and Kynch and the prefaces of the sections are not
of this item and stay. ADR 0086 and EXP-ROT cite the same form of Celik et al. as recalled; they are not among the owned
files and are reported.

How the sources are read: from the publisher's or an author's public copy, the named equations from an image of the page.
For Folden and Aschmoneit arXiv's copy; for Lienhard the PDF of ahtt.mit.edu; for Celik et al. first a copy on www.asme.org,
then the one on qa12.asme.org that the wiki names. The copy's own terms are read and stated in the addendum. No copy is kept
in the repository. On the question the item leaves to the registration, whether the ASME-hosted copy may be cited: a citation
with its locator, and an equation transcribed in our own notation, state a fact about a published page and reproduce none of
its text, so no licence is needed for them and they are made; by the wiki the file states no licence, which the addendum
checks on the copy itself, and a file without a licence is not kept. If the copy forbids being cited, or opens only behind a
sign-in, a payment or a bot check, it is recorded as not read and its mark stays. Nothing is worked around.

## The comparison (registered)

Two routes that share no code. Route A: the source's equation, transcribed from the page image, evaluated in Python with
`decimal` at 60 digits, from the exact value of each binary input. Route B: the repository's code, compiled unchanged: a
scratch crate that depends on `rcfd-fv` by path and calls `Cavitation` and `hx`, and takes the text of `FS` and
`mesh_convergence_estimate` from the test file, printing 17 digits. Both live in the scratch folder. Route A solves Eq. 3 by
scanning (0.001, 20] in 4000 steps for sign changes of `p - RHS(p)` and bisecting, where route B iterates the fixed point.

**Points.**

* S, cavitation: two media, (rho_l, rho_v, n, p_sat) = (1000, 0.02, 1e11, 2339) and (958, 0.6, 1.6e13, 101325), factors 1;
  `alpha_v` in {4.2e-4, 0.013, 0.3, 0.5, 0.87, 0.999}; `p - p_sat` in {-9000, -1, 1, 250, 1e6} Pa (60 points);
  `R` in {1e-6, 1e-5, 3e-5, 1e-4, 1e-3} m for `alpha_v(R)` (10 points).
* L, exchanger: `NTU` in {0.1, 0.5, 1, 1.5, 3, 8}, `Cr` in {0.1, 0.25, 0.5, 0.75, 0.9, 0.99}, both arrangements (72 points);
  `Cr = 0` and `Cr = 1`, the same NTU, both arrangements (24 points; route A takes the value of the equation at `Cr = 0`
  and, for `Cr = 1`, at `1 - 1e-25`). Duty: (C_p, C_s, UA, T_p, T_s) = (20, 40, 30, 300, 350), (40, 20, 30, 300, 350),
  (20, 40, 30, 350, 300), (55, 30, 12, 283, 361), (20, infinite, 30, 300, 350), both arrangements (10 cases). L4: the source's
  Example 3.5 (`NTU` 1.5, `Cr` 0.5, parallel).
* G, mesh convergence, (f1, f2, f3), (r21, r32), cells passed as `((r21 r32)^3, r32^3, 1)`: G1 the recorded torques
  (2.098164, 2.028692, 1.918189) and G2 the pressure rises (73.988, 64.233, 46.778), both with cells (38184, 13627, 4920);
  G3 `3 + 0.7 h^0.7`, h = 1, 1.41, 1.41 x 1.33; G4 `3 + 0.7 h^1.6`, h = 1, 1.49, 1.49 x 1.33; G5 (1, 1.1, 1.05), (1.4, 1.4);
  G6 (1, 1.1, 1.03), (1.5, 1.35); G7 the source's sample, (6.063, 5.972, 5.863), (1.5, 4/3).

**What counts as agreement.** A form agrees with the source if (a) the addendum shows the algebraic identity term by term
(each term of the source named with the term of the code, no term left over on either side), and (b) at every point the two
routes are equal within the limit below. It **differs** if (a) or (b) fails; the difference is then stated with its term and,
where it is a number, its size at the points. A rule that the code adds outside the source (a floor, a clamp, a cap, a factor
whose default is 1, a limit branch) is a difference of scope: it is recorded as "agrees, with" and the rule named, not
silently as agreement. Two are known from the reading of the code: the cap `min(p, 1)` of `U_num`, which makes the code's
value at `p > 1` not that of Eq. 7, and the branch `abs(1 - Cr) < 1e-9`.

## Checks

| Id | Clause | Check | Limit | Source of the limit | Estimate | Margin | Kind |
|---|---|---|---|---|---|---|---|
| R1 | source read | Folden and Aschmoneit, Eqs. 43 and 45 read from the page image; terms of use stated | read and located | acceptance of F-886 | read | n/a | code verification |
| R2 | source read | Lienhard, Eqs. 3.17, 3.20 and 3.21 read from the page image; terms of use stated | read and located | acceptance of F-886 | read | n/a | code verification |
| R3 | source read | Celik et al., Eq. 7 read from the page image; terms of use stated | read and located | acceptance of F-886 | read | n/a | code verification |
| S1 | Schnerr-Sauer | Eq. 43 against `alpha_of_radius`, and its inverse against `radius` | 1e-12 relative at every point | tolerances, "Same discrete problem by two routes", only rounding between them | 1e-15 | 1000 | code verification |
| S2 | Rayleigh rate | Eq. 45 against `radius_rate`, with the sign of `p_sat - p` | 1e-12 relative at every point | as S1 | 1e-15 | 1000 | code verification |
| S3 | Schnerr-Sauer | the mass source, Eqs. 46 and 47 (44 with 45 if they are not there), against `rate` | 1e-12 relative at every point | as S1 | 1e-15 | 1000 | code verification |
| L1 | NTU relations | Eq. 3.20 against `Parallel`, with `Cr = 0` and `Cr = 1` | 1e-12 relative at every point | as S1 | 1e-14 | 100 | code verification |
| L2 | NTU relations | Eq. 3.21 against `Counterflow`, with `Cr = 0` and the limit `Cr -> 1` | 1e-12 relative at every point | as S1 | 1e-13 | 10 | code verification |
| L3 | effectiveness | Eqs. 3.16 and 3.17 from the two outlet temperatures of `solve_exchanger` against its `eps`; its `q`, `ntu` and `cr` against `eps C_min dT`, Eq. 3.18 and the definition | 1e-12 relative at every case | as S1 | 1e-14 | 100 | code verification |
| L4 | effectiveness | the source's Example 3.5 from the code | half a unit of the printed digit, 0.596 +- 0.0005 | tolerances, "Agreement with a published value", the published digits | 0.596 | n/a | code verification |
| G1 | GCI | Eq. 3 against the order `mesh_convergence_estimate` returns on the seven triplets: it is a root that route A finds; the number of roots is reported | 1e-8 relative at every triplet | tolerances, "Same discrete problem by two routes" | 1e-12 | 1e4 | code verification |
| G2 | GCI | Eq. 7 times `abs(phi_1)` against the half-width, at the triplets with `p <= 1` | 1e-8 relative | as G1 | 1e-12 | 1e4 | code verification |
| G3 | GCI | at the triplets with `p > 1` the half-width is `1.25 abs(e21)/(r21 - 1)`; its ratio to Eq. 7 times `abs(phi_1)` is reported, a known difference | 1e-8 relative on the first; no limit on the ratio | as G1 | 1e-12 | 1e4 | code verification |
| G4 | GCI | the source's sample (G7): p 1.53, phi_ext 6.1685, GCI 2.2 % by route A, and the code's order | half a unit of each printed digit | tolerances, "Agreement with a published value", the published digits | equal | n/a | code verification |
| N | negative controls | the eight below, each against its row | largest relative difference over the row's points at least 1e-3, and the row's limit exceeded | tolerances, the sentence on negative controls, with the limit of the controlled row | 0.1 or more | n/a | code verification |
| D1 | no code change | `git diff --stat origin/main` names documents only, no file under `crates`, `apps` or `scripts`; `baseline_hashes.rs` is among the files unchanged | 0 files | acceptance of F-886 | 0 | n/a | unchanged default output |
| D2 | struck, not deleted | each of the five lines, with the marks `~~` and every added text in round brackets that begins ` (ADR 0796` removed, is the line of origin/main byte for byte; no other line of the two fragments changed | 0 differences | acceptance of F-886 | 0 | n/a | unchanged default output |
| D3 | reference-data checks | the two the repository has: (i) `process_report.py --check`, whose generated page `docs/reference-data.md` must be the fragments joined: not current on the branch, which does not write it, and current after `--write` on a scratch export, which must then contain the new fragment; (ii) `cargo test -p rcfd-cli --release --test docs_site`, the links of every page; the links of the new fragment by a script against the files | (i) no failure but a generated file not current; (ii) passes, 0 broken links | brief, task 4; rule 7 | as stated | n/a | unchanged default output |
| P | process | `python scripts/roadmap_proposals.py check`; `python scripts/process_report.py --check` | valid; no failure but a generated file not current | rule 7; rule 9 | as stated | n/a | code verification |

No row is of kind validation: the acceptance claims no agreement with a measured case. `fmt`, `clippy`, unit tests and
`baseline_hashes` are not run, because no source file is touched (D1 shows it). `docs_site` is run on this text before its
commit and once more, as row D3, on the final text. Each comparison script is run once.

### Negative controls and planned mutations (rule 16)

Each is one term of the source's transcription (route A) changed and compared with the code as it is. It must not agree.

| Id | Change | Must fail |
|---|---|---|
| N1 | Eq. 43 without the `1 +` of the denominator (`alpha = x`) | S1 |
| N2 | Eq. 45 with the 3 of `2/(3 rho_l)` dropped | S2 |
| N3 | the mixture density replaced by `rho_l` in the mass source | S3 |
| N4 | Eq. 3.20 with `(1 - Cr)` for `(1 + Cr)` in the exponent | L1 |
| N5 | Eq. 3.21 without the factor `Cr` in the denominator | L2 |
| N6 | Eq. 3.17 with `C_max` for `C_min` in the denominator | L3 |
| N7 | Eq. 3 without `q(p)` | G1 |
| N8 | Eq. 7 with 3 for 1.25 | G2 |

This work has no test, so no mutation patch is committed beside one; the eight above are the registered mutations.

## Form of the addenda

Each of ADR 0039, 0056 and 0762 gets a dated addendum at its end with the columns of the lead's "Sources read" table
(citation, status, where, finding) and the term-by-term table. The heading `Sources read (date)` closes an ADR for the count
of citations from memory ([lead-process.md](../process/lead-process.md)) and asks for a row per citation the ADR makes so.
It is used in ADR 0762 and in this ADR, whose citations are then all in the table. ADR 0039 and 0056 cite more than this item
reads (shell and tube; Manninen, Schiller-Naumann, Kynch, the vapour pressure): there the heading is an addendum's and the
two ADRs stay open in that count. The fragment `docs/reference-data.d/0796-F-886.part` has one row per formula.

Size (agent): done; 0; 0; 1; 250000

## Decision rule

**done** if the three sources were read and each comparison is recorded with its result, agreement or difference; **partial**
if a source could not be read, with the one named. (The brief's rule.) A missed limit is a recorded difference and does not
hold the item by itself; a defect in the set-up of a script is corrected by an amendment before the corrected run (rule 2).

## What is not covered

- Schnerr and Sauer (2001), Rayleigh (1917), Plesset (1949), Kays and London, and Roache's own papers, which are not read.
- The host's use of the cavitation source (radius update, floors, pressure coupling), the crossflow and the shell-and-tube
  relations, and the credibility of the F-870 band.
- A repair: whether a difference found is repaired is a question for the maintainer.

## Amendment 1 (2026-11-25, after the sources were opened, before any comparison script was written or run)

The three sources were opened after the registration was committed (7fc1da85) and their equations read from page images. No
number has been computed. No limit, point, row or the decision rule above is changed. The reading shows three things about
the set-up of the comparison, which are fixed here before its run.

1. **S3, as registered, is expected to be missed, and it is run as registered.** Eqs. 46 and 47 of Folden and Aschmoneit print
   `4 n0 pi R^2/(1 + x) sqrt(2 abs(p_v - p)/(3 rho_l))`, `x = (4/3) n0 pi R^3`. There is no density in them: they are the right-hand
   side of Eq. 44, `d alpha_v/dt + div(alpha_v u)`, in 1/s, where `rate` is a mass source in kg/(m^3 s). Beside S3 two rows are
   registered that say what the difference is. With `rho_m = (rho_l + x rho_v)/(1 + x)` and `alpha_v = x/(1 + x)`, `rate` is
   `rho_v (rho_l/rho_m) 4 n0 pi R^2 R'/(1 + x)^2 = rho_v [4 n0 pi R^2 R'/(1 + x)] / (1 + x rho_v/rho_l)`. The bracket is Eq. 46.
   So the code's source is `rho_v` times the source's divided by `1 + x rho_v/rho_l`: equal in the limit of a small
   `x rho_v/rho_l`, and smaller at a large `x`. The size at the points is reported.
2. **N3 can not be applied as registered:** the printed source has no mixture density. It stays in the table as not
   applicable and is replaced by N3b, on the row S3b.
3. **G4 gets the two other columns of the source's Table 1**, seen in the reading (the wiki did not have them), as G8 and G9:
   (10.7880, 10.7250, 10.6050) and (6.0042, 5.9624, 6.0909), cells per (r21 r32)^3, r32^3, 1 with r21 = 2 and
   r32 = (4500/980)^(1/2). They are known results and not unseen points. They enter row G4b only, not G1 to G3.

| Id | Clause | Check | Limit | Source of the limit | Estimate | Margin | Kind |
|---|---|---|---|---|---|---|---|
| S3b | Schnerr-Sauer, the difference of S3 | `rate` against `rho_v` times Eq. 46 (evaporation) or minus `rho_v` times Eq. 47 (condensation), divided by `1 + x rho_v/rho_l` | 1e-12 relative at every point | tolerances, "Same discrete problem by two routes", only rounding between them | 1e-15 | 1000 | code verification |
| S3c | Schnerr-Sauer, the difference of S3 | the ratio `rate/(rho_v Eq. 46)` at the points, and at the venturi of ADR 0056 (n = 1e11, rho_l 1000, rho_v 0.02; `alpha_v` 0.9, 0.99, 0.999, 0.9999), by route A | none, reported | acceptance of F-886 (a difference is a finding) | 0.6 to 1 | n/a | code verification |
| G4b | GCI | G8 and G9, Table 1, columns 2 and 3: p 0.75 and 1.51, phi_ext 10.8801 and 6.0269, GCI 1.1 % and 0.5 % by route A, and the code's order | half a unit of each printed digit | tolerances, "Agreement with a published value", the published digits | equal | n/a | code verification |

| Id | Change | Must fail |
|---|---|---|
| N3b | Eq. 46 with `(1 + x)^2` for `(1 + x)` in the denominator (the material derivative taken for the conservative one) | S3b |

N3b has the limit of row N.
