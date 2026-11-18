# Tolerance table

Status: PROVISIONAL VERSION 1, approved by the maintainer on 2026-11-18. It binds registrations written under
[agent-rules.md](agent-rules.md) from that date and no earlier one: a limit in such a registration cites a row here, a
published reference with its place in it, or the acceptance of the parent item (rule 15). Rows that are missing are added
from the registrations that need them, each addition approved like this table.

The starting values are not new numbers. They are the limits found in the check tables of 18 registrations read for this
draft: ADR 0357, 0365, 0378, 0379, 0394, 0501, 0504, 0527, 0558, 0569, 0573, 0575, 0602, 0614, 0620, 0627, 0672 and 0682.
That is a small sample, mostly of the structural solver, so each row needs the maintainer's eye before it is a rule. A
registration may always set a tighter limit than its row.

| Class of check | Kind (rule 14) | Starting value, as seen | Seen in |
| --- | --- | --- | --- |
| Bit-identity of default output | unchanged default output | exact: equal bytes or equal hash | rule 3; ADR 0672 (`0`, bit for bit) |
| Same discrete problem by two routes | code verification | relative difference `<= 1e-8`; `<= 1e-12` where only rounding separates the two | ADR 0575, 0379, 0614 |
| Agreement with a closed form | code verification | relative error `<= 1e-6` to `<= 1e-9` where the closed form is exact for the discrete problem; `<= 1e-3` where it is exact only for the continuum | ADR 0501, 0378, 0558, 0394 |
| Observed order of accuracy | solution verification | within 0.5 of the formal order, or at least the formal order less 0.1 | ADR 0569, 0573, 0558 |
| Grid convergence | solution verification | ratio of successive differences within 2.5 % of its formal value (3.9 to 4.1 for order 2); the Richardson value within the row for its reference | ADR 0602, 0573 |
| Linear and eigenvalue solver residual | code verification | relative residual `<= 1e-10` | ADR 0357, 0504 |
| Agreement with a published value | solution verification | relative error `<= 5e-3` against a value published to four digits; the published digits where the same discrete problem is reproduced | ADR 0627, 0682 |
| Agreement with a result on a known front or optimum | solution verification | `<= 2 %` | ADR 0620 |
| Agreement with a measured case | validation | none seen in the sample; to be set by the maintainer per measured case | |
| Run time or memory, as a budget | unchanged default output | a stated budget in seconds or bytes with the estimate beside it; a ratio `<= 0.5` where a speed-up is the claim | ADR 0365, 0527 |

A negative control states its limit the other way round (the difference must be at least a stated size) and takes it from
the row of the check it controls.

Not covered by the sample: conservation of mass, momentum or energy in flow cases, and turbulence-model comparisons with
published profiles. Rows for them are added from the registrations that use them before approval.
