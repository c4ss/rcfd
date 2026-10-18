//! Non-dominated sorting for minimisation (Deb et al. 2002, "fast non-dominated sort") with constrained domination
//! (Deb 2000): a feasible point dominates any infeasible one, two infeasible points compare by violation, two feasible
//! points by Pareto dominance.

/// Whether point `a` (objectives, constraint violation) dominates `b`.
pub fn dominates(a: &[f64], va: f64, b: &[f64], vb: f64) -> bool {
    let (fa, fb) = (va <= 0.0, vb <= 0.0);
    match (fa, fb) {
        (true, false) => true,
        (false, true) => false,
        (false, false) => va < vb,
        (true, true) => {
            let mut strictly = false;
            for (x, y) in a.iter().zip(b) {
                if x > y {
                    return false;
                }
                if x < y {
                    strictly = true;
                }
            }
            strictly
        }
    }
}

/// Fronts of non-dominated sorting: `fronts[0]` is the non-dominated set; each front lists indices in ascending order.
/// `violation[i] <= 0` means feasible; non-finite objectives are treated as infinitely violated.
pub fn non_dominated_sort(objs: &[Vec<f64>], violation: &[f64]) -> Vec<Vec<usize>> {
    let n = objs.len();
    let viol: Vec<f64> = (0..n)
        .map(|i| {
            if objs[i].iter().any(|x| !x.is_finite()) {
                f64::INFINITY
            } else {
                violation.get(i).copied().unwrap_or(0.0)
            }
        })
        .collect();
    let mut dominated_by_count = vec![0usize; n];
    let mut dominating: Vec<Vec<usize>> = vec![Vec::new(); n];
    for i in 0..n {
        for j in (i + 1)..n {
            if dominates(&objs[i], viol[i], &objs[j], viol[j]) {
                dominating[i].push(j);
                dominated_by_count[j] += 1;
            } else if dominates(&objs[j], viol[j], &objs[i], viol[i]) {
                dominating[j].push(i);
                dominated_by_count[i] += 1;
            }
        }
    }
    let mut fronts = Vec::new();
    let mut current: Vec<usize> = (0..n).filter(|&i| dominated_by_count[i] == 0).collect();
    while !current.is_empty() {
        let mut next = Vec::new();
        for &i in &current {
            for &j in &dominating[i] {
                dominated_by_count[j] -= 1;
                if dominated_by_count[j] == 0 {
                    next.push(j);
                }
            }
        }
        current.sort_unstable();
        fronts.push(std::mem::take(&mut current));
        current = next;
    }
    fronts
}

/// The first front only.
pub fn front(objs: &[Vec<f64>], violation: &[f64]) -> Vec<usize> {
    non_dominated_sort(objs, violation)
        .into_iter()
        .next()
        .unwrap_or_default()
}

/// Crowding distance of the members of one front (Deb et al. 2002); boundary points get infinity.
pub fn crowding_distance(objs: &[Vec<f64>], members: &[usize]) -> Vec<f64> {
    let m = members.len();
    let mut dist = vec![0.0; m];
    if m <= 2 {
        return vec![f64::INFINITY; m];
    }
    let d = objs[members[0]].len();
    for k in 0..d {
        let mut order: Vec<usize> = (0..m).collect();
        order.sort_by(|&a, &b| objs[members[a]][k].total_cmp(&objs[members[b]][k]));
        let (lo, hi) = (objs[members[order[0]]][k], objs[members[order[m - 1]]][k]);
        dist[order[0]] = f64::INFINITY;
        dist[order[m - 1]] = f64::INFINITY;
        if hi > lo {
            for w in 1..m - 1 {
                dist[order[w]] += (objs[members[order[w + 1]]][k] - objs[members[order[w - 1]]][k]) / (hi - lo);
            }
        }
    }
    dist
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::design::rng::Rng;

    #[test]
    fn p1_zdt1_like_front_is_extracted_exactly() {
        let mut rng = Rng::new(7);
        let mut pts: Vec<(Vec<f64>, bool)> = Vec::new();
        for i in 0..200 {
            let f1 = i as f64 / 199.0;
            pts.push((vec![f1, 1.0 - f1.sqrt()], true));
        }
        for _ in 0..300 {
            let f1 = rng.uniform();
            pts.push((vec![f1, 1.0 - f1.sqrt() + 0.05 + 0.1 * rng.uniform()], false));
        }
        Rng::new(11).shuffle(&mut pts);
        let objs: Vec<Vec<f64>> = pts.iter().map(|p| p.0.clone()).collect();
        let viol = vec![0.0; objs.len()];
        let fronts = non_dominated_sort(&objs, &viol);
        let want: Vec<usize> = (0..pts.len()).filter(|&i| pts[i].1).collect();
        assert_eq!(fronts[0], want);
        assert_eq!(want.len(), 200);
        let total: usize = fronts.iter().map(Vec::len).sum();
        assert_eq!(total, 500);
        assert!(fronts.len() > 1);
        assert_eq!(front(&objs, &viol), want);
    }

    #[test]
    fn quarter_circle_and_three_objective_plane() {
        let mut pts: Vec<(Vec<f64>, bool)> = Vec::new();
        for i in 0..=100 {
            let t = i as f64 / 100.0 * std::f64::consts::FRAC_PI_2;
            pts.push((vec![t.cos(), t.sin()], true));
            pts.push((vec![t.cos() + 0.02, t.sin() + 0.02], false));
        }
        let objs: Vec<Vec<f64>> = pts.iter().map(|p| p.0.clone()).collect();
        let f = front(&objs, &vec![0.0; objs.len()]);
        assert_eq!(f, (0..objs.len()).filter(|&i| pts[i].1).collect::<Vec<_>>());
        // f1 + f2 + f3 = 1 plane with exactly representable steps of 0.25: nothing dominates anything
        let plane: Vec<Vec<f64>> = (0..=4)
            .flat_map(|i| (0..=(4 - i)).map(move |j| vec![i as f64 / 4.0, j as f64 / 4.0, (4 - i - j) as f64 / 4.0]))
            .collect();
        let n = plane.len();
        assert_eq!(front(&plane, &vec![0.0; n]).len(), n);
    }

    #[test]
    fn duplicates_and_constraint_domination() {
        let objs = vec![vec![1.0, 1.0], vec![1.0, 1.0], vec![2.0, 2.0]];
        assert_eq!(front(&objs, &[0.0, 0.0, 0.0]), vec![0, 1]);
        // a feasible bad point dominates an infeasible good one; two infeasible compare by violation
        let objs = vec![vec![10.0, 10.0], vec![0.0, 0.0], vec![0.0, 0.0]];
        let fr = non_dominated_sort(&objs, &[0.0, 0.5, 0.25]);
        assert_eq!(fr, vec![vec![0], vec![2], vec![1]]);
        // a failed (non-finite) evaluation is last
        let objs = vec![vec![f64::NAN, 0.0], vec![5.0, 5.0]];
        assert_eq!(non_dominated_sort(&objs, &[0.0, 0.0]), vec![vec![1], vec![0]]);
    }

    #[test]
    fn crowding_distance_marks_boundaries() {
        let objs = vec![vec![0.0, 1.0], vec![0.5, 0.5], vec![1.0, 0.0]];
        let d = crowding_distance(&objs, &[0, 1, 2]);
        assert!(d[0].is_infinite() && d[2].is_infinite());
        assert!((d[1] - 2.0).abs() < 1e-12);
    }
}
