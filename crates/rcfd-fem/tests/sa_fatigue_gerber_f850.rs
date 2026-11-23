//! F-850 (ADR 0778), check (4): the refusal texts of a positive mean stress (G2 of ADR 0453: Goodman and Gerber at
//! `sigma_u`, Soderberg at `sigma_y`) asserted by equality. ADR 0453 registered `==` and its test `sa_fatigue_f534.rs`
//! asserts containment; row G2= of ADR 0778.

use rcfd_fem::fatigue::{MeanStress, MeanStressCorrection};
use rcfd_fem::FemError;

const SU: f64 = 600e6;
const SY: f64 = 400e6;

#[test]
fn g2_positive_mean_refusals_are_the_exact_text() {
    let cases = [
        (
            "Goodman",
            MeanStressCorrection::new(MeanStress::Goodman { ultimate: SU }),
            SU,
            "mean stress 6e8 at or above the strength 6e8 (Goodman): no equivalent amplitude",
        ),
        (
            "Gerber",
            MeanStressCorrection::new(MeanStress::Gerber { ultimate: SU }),
            SU,
            "mean stress 6e8 at or above the strength 6e8 (Gerber): no equivalent amplitude",
        ),
        (
            "Soderberg",
            MeanStressCorrection::new(MeanStress::Soderberg { yield_strength: SY }),
            SY,
            "mean stress 4e8 at or above the strength 4e8 (Soderberg): no equivalent amplitude",
        ),
    ];
    for (name, k, strength, literal) in cases {
        let sm = strength;
        let e = k
            .equivalent_amplitude(100e6, sm)
            .expect_err("G2=: a mean at the strength must be refused");
        let FemError::InvalidValue { key, reason } = &e else {
            panic!("G2= {name}: not an InvalidValue: {e}");
        };
        // The pattern of ADR 0453 ("the exact present text, formatted in the test from the same pattern").
        let formatted =
            format!("mean stress {sm:e} at or above the strength {strength:e} ({name}): no equivalent amplitude");
        assert_eq!(*key, "mean stress", "G2= {name}: the key");
        assert_eq!(reason, &formatted, "G2= {name}: the reason against the formatted text");
        assert_eq!(reason, literal, "G2= {name}: the reason against the literal text");
        println!("G2= {name}: met ({reason})");
    }
}
