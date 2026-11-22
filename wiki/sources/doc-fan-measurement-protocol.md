---
type: source
tags: [fan, measurement, computer-fan, validation]
sources: []
origin: docs/fan-measurement-protocol.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Fan measurement protocol (computer fans)

**Origin:** [docs/fan-measurement-protocol.md](../../docs/fan-measurement-protocol.md) at origin/main 2e94e5ef
**Kind:** repo doc: measurement protocol (2026-10-26, wave 46E, part G2 input). "This is a minimum protocol, not a standard".

## Summary

Lists what a user must supply for a fan they measure, so the computer-fan tools of [[doc-fan-project-breakdown]] can be checked against it; written because the survey in [[doc-validation-data]] found no open source adequate as a computer-fan reference. Scope: one axial or centrifugal fan of 40 to 140 mm, 1 000 to 5 000 rpm. Accuracy rule: each measurement's standard uncertainty should be at most one third of the comparison tolerance. "The tolerance is not set yet. This page assumes 5 % on pressure rise and flow, so about 1.7 % per measurement." The limit registered in the ADR of the first comparison replaces this assumption.

## Key points

- Tier 0 (required): identification; rotor sections at no fewer than 7 radii, preferably a 3D scan with 0.05 mm accuracy or better; tip gap at no fewer than 4 circumferential positions per blade; frame; rotor mass (0.1 g). Option: print a rotor generated from `rcfd_mesh::fan` parameters so the geometry is known by construction.
- Tier 1 fan curve (required): plenum test in the style of ISO 5801 / ANSI/AMCA 210 (NOT read) or ISO 10302-1:2024 (NOT read); at least 10 points per speed, at least 3 speeds (for example 1 000, 1 500 and 2 000 rpm for a 120 mm fan). Targets: speed 0.25 %; pressure 1 % of reading plus 0.5 Pa (scale rho U_tip^2 "about 190 Pa for 120 mm at 2 000 rpm (U_tip = 12.6 m/s)"); flow 1.5 %; ambient pressure 0.5 kPa, temperature 1 K, humidity 5 %; supply voltage and current 1 % each (efficiency is wire-to-air).
- Torque is "optional and usually impractical": a 120 mm fan at 2 000 rpm needs about 1.9 W of shaft power, about 9 mN m; "Without torque, shaft power is not validated; say so in any comparison."
- Tier 1b (strongly recommended): one system impedance curve (heat sink, filter or grille) and the operating point of the fan on it, "the quantity a computer-fan user cares about and the one C5 predicts".
- Tier 2 noise (recommended): sound power at every curve point of at least one speed; ECMA-74 reproducibility of about 1.5 dB is the target; background at least 10 dB below; raw signals 48 kHz, 24 bit, at least 30 s with a tach channel; blade-passing frequency B n / 60 (233 Hz for 7 blades at 2 000 rpm).
- Tier 3 vibration (optional): unbalance force U omega^2, "1 g mm at 2 000 rpm is 0.044 N; the NASA Quiet Space Fan's 0.762 g mm at 12 000 rpm is 1.2 N".
- Data delivery: one CSV per speed, header `variant,point,rpm,flow_m3_s,dp_static_Pa,dp_side,supply_V,supply_A,p_amb_Pa,T_amb_K,rh,lwa_dB,notes`; `variant` matches design candidates in the fidelity driver (`design::fidelity::MeasuredTable`, ADR 0622). "Measured files stay outside the repository; only aggregate calibration residuals are published."

## Contradictions with the wiki

None found.

## Pages this source informs

- [[computer-fan-target]], [[design-ledger]], [[design-loop]], [[verification-vs-validation]]
