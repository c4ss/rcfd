---
type: source
tags: [adr-thread, validation, milestones, naca0012, cylinder, roadmap]
sources: [doc-milestones-json, doc-roadmap-overview, doc-reference-data-f868-ladson-naca0012, doc-reference-data-f869-cylinder-mean-drag]
origin: docs/adr/ (ADR list below)
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# ADR thread: simulation milestones and their validation headlines (F-865, F-867, F-868, F-869)

**Origin:** the ADRs in the table below, `docs/adr/` at `origin/main` 2e94e5ef.
**Kind:** ADR thread (registrations with result addenda, the lead's readings and the maintainer's answers), 5 ADRs, 2026-11-21 to 2026-11-22.

How the roadmap came to measure "per cent complete" per named simulation (F-865, ADR 0756), how the fan milestone was widened to the whole design goal (F-867, ADR 0757), and the first two registered comparisons that the maintainer's answers to ADR 0756 created as headline items: the NACA 0012 lift curve against Ladson (F-868, ADR 0760) and the mean drag of the cylinder at Re 100 (F-869, ADRs 0761 and 0763). The third headline created the same day, the FAN-01 design point (F-870, ADR 0762), has its own page: [[adr-thread-fan01-design-point]].

## Current state

- **Milestone figures exist and are generated.** F-865 is done (ADR 0756): `docs/milestones.json` defines the milestones, memberships are written into `docs/roadmap.json` (61 memberships of 32 items at the merge) and a block of `docs/roadmap-overview.md` is generated. The three per cents at the merge were 92, 89 and 90; the lead wrote that they "are high because the members of a milestone are ... what its run of record uses", so "The column 'Headline result' is the one to read" (ADR 0756, the lead's reading).
- **The fan milestone is the whole design goal** (F-867 done, ADR 0757): 89 members, 49 done, 30 partly done, 10 not started, 72 per cent, headline not met at the merge. Headline from 2026-11-21: F-870, F-872, F-873, F-260, F-702 and F-874 (ADR 0757, answers of the maintainer).
- **New headline items from the maintainer's answers of 2026-11-21 to ADR 0756** (questions 1 to 3, option (b) each): F-868 (NACA 0012 lift curve), F-869 (cylinder mean drag), F-870 (FAN-01 design point). The six further milestones of question 4 are approved and are item F-871 (todo). With F-869 entered, the vortex-shedding headline "reads 'partly met' from this day, where it read 'met' on the Strouhal number alone" (ADR 0756).
- **F-868 partial** (ADR 0760): steady SA on mesh M1w-far500 puts the lift inside the band max(5 %, 0.018) of Ladson's 80-grit data at all eleven angles 0 to 16 degrees (largest difference 0.0143 at 16) and gives a maximum of 1.542, 4.3 % under the measured 1.6116; 17 degrees is a known miss (no steady value; iteration mean 1.0476 against 1.6057). The maintainer answered question 1 with option (a) on 2026-11-22: F-868 stays partial and an unsteady arm at 17 degrees is registered under F-868, "issued as ADR 0764" (not in this snapshot).
- **F-869 partial** (ADRs 0761, 0763): on the registered sequence at dt 0.025 with the far field at 30 D the fine-mesh drag 1.33900 (181 x 141) is inside the band [1.314, 1.370], and the Richardson value is inside it for every order from 1 to 2.5 (1.343 to 1.354), but the observed order is 1.387, outside the registered [1.5, 2.5], so the mesh-converged drag "is not shown" (known miss W3b). ADR 0763 question 1 is open with the maintainer; the agent and the lead both recommend (c).
- The comparison of F-869 uses the option `time_scheme = Bdf2`; the default first-order step gives 1.30996 at the same mesh and step, outside the band. By the maintainer's answer to ADR 0761 question 2 (option (a)), F-869 is accepted with the option and "the default of the transient solver is not changed and no item is entered for it".

## ADRs

| ADR | Date | Kind | Item | Question | Outcome | Superseded by |
|---|---|---|---|---|---|---|
| [0756](../../docs/adr/0756-f865-registration-per-cent-complete-for-the-roadmap-and-for-each-simulation-milestone.md) | 2026-11-21 | registration (hub) | F-865 | Per cent complete for the roadmap and for each simulation milestone | done; 92 / 89 / 90 per cent at the merge; Q1 to Q4 answered (b), (b), (b), first six milestones | |
| [0757](../../docs/adr/0757-f867-registration-the-fan-milestone-as-the-whole-design-goal.md) | 2026-11-21 | registration | F-867 | The fan milestone as the whole fan design goal | done; 89 members, 72 per cent; Q1 to Q5 answered, F-872 to F-875 entered | |
| [0760](../../docs/adr/0760-f868-registration-naca0012-lift-curve-against-ladson.md) | 2026-11-22 | registration | F-868 | NACA 0012 lift at Re 6e6 against Ladson within max(5 %, 0.018) | partial; V00 to V16 and VM met, V17 known miss (1.0476 against 1.6057) | ADR 0764 (issued, not in snapshot) |
| [0761](../../docs/adr/0761-f869-registration-the-mean-drag-of-the-cylinder-at-re-100-against-the-published-spread.md) | 2026-11-22 | registration | F-869 | Cylinder mean drag at Re 100 against the published spread [1.314, 1.370] | partial; V1 1.35247 met, V2b Richardson 1.37479 known miss | 0763 |
| [0763](../../docs/adr/0763-f869-registration-the-mesh-converged-mean-drag-of-the-cylinder-at-re-100.md) | 2026-11-22 | registration (follow-up under rule 13) | F-869 | Mesh-converged drag at dt 0.025, far field 30 D | partial; W1 1.33900 met, W3b observed order 1.387 known miss | |

## Key numbers and limits

- **Milestone definitions** (ADR 0756, 0757): headline items per milestone in `docs/milestones.json`: F-029 and F-869 (vortex shedding); F-043 and F-868 (aerofoil lift); F-870, F-872, F-873, F-260, F-702 and F-874 (fan) ([[doc-milestones-json]]).
- **F-868 band** (ADR 0760): `abs(cl - cl_L(alpha)) <= B(alpha) = max(0.05 abs(cl_L(alpha)), s), s = 0.018`. 0.05 is "the cl band of ADR 0167's LT10 and LT15"; s is the scatter of the three grit zones. Angles compared: 0, 2, 4, 6, 8, 10, 12, 13, 14, 15, 16 and 17 degrees; 18, 20, 22 and 24 are named as not compared. "A value that is not a flow value is a miss" (exit code 0, or ADR 0167's D5: window range of `cl_eff` <= 0.005 and of `cd_eff` <= 0.0005).
- **F-868 result table** (ADR 0760 addendum 1): V00 -0.0000 against -0.0072; V02 0.2202 against 0.2071; V10 1.0656 against 1.0586; V14 1.4235 and V15 1.4932 at the 8000-iteration cap (exit code 5) but flow values; V16 1.5420 against 1.5563 (difference -0.0143, band 0.0778); V17 1.0476 iteration mean against 1.6057, band 0.0803, "over its last 1000 iterations cl swings between -0.066 and 1.955". Twelve runs, 46 minutes in all; binary sha256 `00848da110de638130b0dfa2c9af197b04fa34039edf9b7234222aec2ec8b081`.
- **F-868 band for later registrations:** the lead decided ADR 0760 question 2 (his to decide): "a later registration against this measured set takes the scatter of the three grit zones, 0.018, as the band below stall, and not the larger of it and 5 per cent." The lead notes that "from 6 degrees up the 5 per cent is 3.5 to 125 times the difference found".
- **F-869 band** (ADR 0761): "B = [1.314, 1.370]: the smallest and the largest of the eleven published values ... without the row '(Present 32 x 16)'". Width 0.056, middle 1.342, half-width 2.09 %. K2's window of ADR 0060, "within 4 % of 1.342", [1.2883, 1.3957], is kept for the coarse mesh only.
- **F-869 first sequence** (ADR 0761, BDF2, far field 20 D, dt 0.05): 64 x 50 1.31674; 90 x 70 1.33879; 128 x 100 1.35247 (V1, St 0.16480, equal to ADR 0069); 128 x 100 at dt 0.025 1.34511; at far field 30 D 1.33884; Richardson value 1.37479, "0.0048 above the upper end". Arithmetic, not a check: halving dt lowers the drag by 0.00736; the far field at 20 D raises it by 0.0136 against 30 D.
- **F-869 second sequence** (ADR 0763, BDF2, dt 0.025, far field 30 D, last 8 whole periods): 90 x 70 1.32259 (St 0.16169); 128 x 100 1.33285 (St 0.16395); 181 x 141 1.33900 (St 0.16486). Ratios 1.42539 and 1.41203; observed order 1.387 against [1.5, 2.5]. Formula values printed "as registered and not results": 1.34901 at the observed order, 1.34519 at order 2; GCI_fine 0.935 %. "It reaches the upper end 1.370 only for an order under 0.52."
- **F-869 reading of the order** (ADR 0763): the order "is the same as on the sequence of ADR 0761 (1.38 at dt 0.05 and 20 D...)", so "the expectation of the registration, that the time error held the order down, is therefore not supported: the low order looks like a property of the spatial discretisation". Candidates named and not examined: "the strongly graded O-grid, the wall force integral, the far-field condition".

## Open questions

- **ADR 0763 question 1 (open with the maintainer):** "The fine value and every extrapolated value are in the band, but the observed order of the sequence is 1.39, under the range [1.5, 2.5] that this registration set for a valid extrapolation. Is the item done?" Options: (a) partial, a further registration adds a fourth mesh 256 x 200; (b) done as it stands with the known miss kept as a record; (c) as (b), and a new item diagnoses why the drag converges at order 1.4. Recommendation (c), by the agent and by the lead at the merge. No "Answers of the maintainer" section is in ADR 0763 at this snapshot.
- ADR 0760 question 1 ("Does the headline need 17 degrees?") is **answered** (option (a), 2026-11-22), although the roadmap gap of F-868 and [[doc-roadmap-overview]] still name it as the maintainer's question. See the contradiction note in [[naca0012]].
- Not read by the agent of ADR 0760, "to be closed by a citation batch": NASA TM 4074 itself (the lead's reading).
- ADR 0756 question 4 left six proposed milestones (conjugate heat transfer and five others) waiting "until a headline item exists for each".

## Related

- [[simulation-milestones]], [[naca0012]], [[cylinder-re100]], [[fan-01]], [[gci-richardson]], [[verification-vs-validation]], [[known-miss]]
- [[adr-thread-fan01-design-point]], [[adr-thread-process-measures-and-roadmap-form]]
- [[roadmap-quality-docs-and-validation]]
