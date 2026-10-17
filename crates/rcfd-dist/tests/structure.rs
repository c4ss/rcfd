//! ADR 0058 checks B1 (partition structure), B2 (located errors), B3 (halo update), B5a (rank-ordered reduction).

mod common;

use common::*;
use rcfd_dist::{run_ranks, validate_decomposition, Communicator, DistCsr, HaloPlan, SerialComm};
use rcfd_mesh::mesh::Mesh;
use rcfd_mesh::partition::Decomposition;
use std::collections::BTreeSet;

fn meshes() -> Vec<(&'static str, Mesh)> {
    vec![("cavity", cavity(24)), ("box", box8())]
}

/// B1: independent structural check, not reusing the validator.
#[test]
fn b1_partition_structure() {
    for (mname, mesh) in meshes() {
        for (pname, part) in partitioners() {
            for p in [1usize, 2, 4, 8] {
                let dec = Decomposition::new(&mesh, p, part.as_ref()).expect("decomposition");
                let tag = format!("{mname}/{pname}/P={p}");
                // every cell owned exactly once
                let mut count = vec![0usize; mesh.n_cells()];
                for s in &dec.subdomains {
                    for &c in &s.owned {
                        count[c] += 1;
                    }
                }
                assert!(count.iter().all(|&k| k == 1), "{tag}: cell lost or duplicated");
                assert_eq!(
                    dec.subdomains.iter().map(|s| s.n_owned()).sum::<usize>(),
                    mesh.n_cells(),
                    "{tag}"
                );
                for (c, &q) in dec.part_of_cell.iter().enumerate() {
                    assert!(dec.subdomains[q].owned.contains(&c), "{tag}: part_of_cell");
                }
                // ghost set equals the exact off-part neighbour set from the face list
                for s in &dec.subdomains {
                    let mut want = BTreeSet::new();
                    for f in 0..mesh.n_internal_faces() {
                        let (a, b) = (mesh.owner()[f], mesh.neighbour()[f]);
                        if dec.part_of_cell[a] == s.part && dec.part_of_cell[b] != s.part {
                            want.insert(b);
                        }
                        if dec.part_of_cell[b] == s.part && dec.part_of_cell[a] != s.part {
                            want.insert(a);
                        }
                    }
                    let got: BTreeSet<usize> = s.ghosts.iter().copied().collect();
                    assert_eq!(got.len(), s.ghosts.len(), "{tag}: duplicate ghost");
                    assert_eq!(got, want, "{tag}: part {} ghost set", s.part);
                    for &g in &s.ghosts {
                        assert_ne!(dec.part_of_cell[g], s.part, "{tag}: ghost owned by self");
                    }
                    // recv/send agree element by element
                    for (&q, rids) in &s.recv {
                        let sids = &dec.subdomains[q].send[&s.part];
                        assert_eq!(sids.len(), rids.len(), "{tag}: message length {q}->{}", s.part);
                        for (&si, &ri) in sids.iter().zip(rids) {
                            let sg = dec.subdomains[q].owned[si];
                            let rg = s.ghosts[ri - s.n_owned()];
                            assert_eq!(sg, rg, "{tag}: message order {q}->{}", s.part);
                            assert_eq!(dec.part_of_cell[rg], q, "{tag}: ghost peer");
                        }
                    }
                }
                validate_decomposition(&mesh, &dec).unwrap_or_else(|e| panic!("{tag}: {e}"));
            }
        }
    }
}

fn expect_err(mesh: &Mesh, dec: &Decomposition, tokens: &[String]) {
    let e = validate_decomposition(mesh, dec)
        .expect_err("corruption must be rejected")
        .to_string();
    for t in tokens {
        assert!(e.contains(t.as_str()), "error text {e:?} lacks {t:?}");
    }
}

/// B2: five corruptions, each located by part, peer and cell.
#[test]
fn b2_located_errors() {
    let mesh = cavity(16);
    let base = decompose(&mesh, 4);
    validate_decomposition(&mesh, &base).expect("base decomposition is valid");
    // a pair (p, q) with ghosts
    let p = 0usize;
    let q = *base.subdomains[p].recv.keys().next().expect("part 0 has a neighbour");

    // (i) a ghost dropped (ghost list and receive list stay coherent)
    {
        let mut d = base.clone();
        let s = &mut d.subdomains[p];
        let g = s.ghosts.pop().expect("ghost");
        let gq = base.part_of_cell[g];
        let id = s.owned.len() + s.ghosts.len();
        s.recv.get_mut(&gq).expect("recv").retain(|&i| i != id);
        // the matching send entry stays on the owner (it no longer matches), but the dropped ghost is detected first
        expect_err(
            &mesh,
            &d,
            &[format!("part {p}"), format!("peer {gq}"), format!("cell {g}")],
        );
    }
    // (ii) an owned cell duplicated into another part
    {
        let mut d = base.clone();
        let c = d.subdomains[p].owned[0];
        d.subdomains[q].owned.push(c);
        expect_err(
            &mesh,
            &d,
            &[format!("part {q}"), format!("peer {p}"), format!("cell {c}")],
        );
    }
    // (iii) send list shortened
    {
        let mut d = base.clone();
        let sids = d.subdomains[q].send.get_mut(&p).expect("send");
        let dropped = sids.pop().expect("entry");
        let _ = dropped;
        let ghost_cell = {
            let t = &base.subdomains[p];
            let rid = *t.recv[&q].last().expect("recv");
            t.ghosts[rid - t.n_owned()]
        };
        expect_err(
            &mesh,
            &d,
            &[format!("part {p}"), format!("peer {q}"), format!("cell {ghost_cell}")],
        );
    }
    // (iv) a ghost attributed to a wrong peer
    {
        let mut d = base.clone();
        let other = (0..4).find(|&r| r != p && r != q).expect("third part");
        let s = &mut d.subdomains[p];
        let id = s.recv[&q][0];
        let g = s.ghosts[id - s.n_owned()];
        s.recv.get_mut(&q).expect("recv").remove(0);
        s.recv.entry(other).or_default().push(id);
        expect_err(
            &mesh,
            &d,
            &[format!("part {p}"), format!("peer {other}"), format!("cell {g}")],
        );
    }
    // (v) an owned cell removed (lost); pick a cell with a neighbour in another part
    {
        let mut d = base.clone();
        let foreign_parts = |c: usize| -> BTreeSet<usize> {
            let mut set = BTreeSet::new();
            for f in 0..mesh.n_internal_faces() {
                let (a, b) = (mesh.owner()[f], mesh.neighbour()[f]);
                if a == c && base.part_of_cell[b] != p {
                    set.insert(base.part_of_cell[b]);
                }
                if b == c && base.part_of_cell[a] != p {
                    set.insert(base.part_of_cell[a]);
                }
            }
            set
        };
        let ghost_of_q = base.subdomains[q]
            .ghosts
            .iter()
            .copied()
            .find(|&g| base.part_of_cell[g] == p && foreign_parts(g) == BTreeSet::from([q]))
            .expect("p-owned cell touching only q");
        d.subdomains[p].owned.retain(|&c| c != ghost_of_q);
        expect_err(
            &mesh,
            &d,
            &[format!("part {p}"), format!("peer {q}"), format!("cell {ghost_of_q}")],
        );
    }
    // matrix coupling outside the halo: part, row, column are named
    {
        let a = poisson(&mesh);
        let mut triplets = Vec::new();
        for i in 0..a.n {
            for j in a.row_ptr[i]..a.row_ptr[i + 1] {
                triplets.push((i, a.col[j], a.val[j]));
            }
        }
        let row = base.subdomains[p].owned[0];
        let far = (0..mesh.n_cells())
            .find(|&c| base.part_of_cell[c] != p && !base.subdomains[p].ghosts.contains(&c))
            .expect("far cell");
        triplets.push((row, far, -1.0));
        let bad = rcfd_linalg::CsrMatrix::from_triplets(a.n, triplets);
        let e = DistCsr::from_global(&bad, &base, p)
            .expect_err("coupling outside the halo")
            .to_string();
        for t in [
            format!("part {p}"),
            format!("row {row}"),
            format!("global column {far}"),
        ] {
            assert!(e.contains(&t), "error text {e:?} lacks {t:?}");
        }
    }
}

/// B3: after the in-process exchange every ghost equals its owner value bit for bit; also equals `Decomposition::exchange`.
#[test]
fn b3_halo_update() {
    for (mname, mesh) in meshes() {
        let cc = mesh.cell_centres().to_vec();
        let fields: Vec<(&str, Vec<f64>)> = vec![
            ("id", (0..mesh.n_cells()).map(|c| c as f64).collect()),
            (
                "smooth",
                cc.iter().map(|x| (1.7 * x[0]).sin() + x[1] * x[2] + 3.0).collect(),
            ),
        ];
        for (pname, part) in partitioners() {
            for p in [2usize, 4, 8] {
                let dec = Decomposition::new(&mesh, p, part.as_ref()).expect("decomposition");
                for (fname, g) in &fields {
                    let tag = format!("{mname}/{pname}/P={p}/{fname}");
                    let local0 = dec.scatter(g);
                    let results = run_ranks(p, |comm| {
                        let plan = HaloPlan::from_subdomain(&dec.subdomains[comm.rank()]);
                        let mut v = local0[comm.rank()].clone();
                        plan.exchange(comm, &mut v).expect("exchange");
                        v
                    });
                    let mut reference = local0.clone();
                    dec.exchange(&mut reference);
                    for (r, v) in results.iter().enumerate() {
                        let s = &dec.subdomains[r];
                        for (i, &c) in s.owned.iter().enumerate() {
                            assert_eq!(v[i].to_bits(), g[c].to_bits(), "{tag}: owned changed");
                        }
                        for (k, &c) in s.ghosts.iter().enumerate() {
                            assert_eq!(
                                v[s.n_owned() + k].to_bits(),
                                g[c].to_bits(),
                                "{tag}: ghost {c} on part {r}"
                            );
                        }
                        assert_eq!(
                            v.iter().map(|x| x.to_bits()).collect::<Vec<_>>(),
                            reference[r].iter().map(|x| x.to_bits()).collect::<Vec<_>>(),
                            "{tag}: differs from Decomposition::exchange"
                        );
                    }
                }
            }
        }
    }
}

/// B5a: the reduction is the sequential rank-order sum, bit-identical on every rank, and the identity for one rank.
#[test]
fn b5a_reduction_order() {
    let vals = [0.1f64, 1e16, -1e16, 0.3, 7.25e-3, -2.0, 1.0 / 3.0, 5.5];
    for p in [2usize, 4, 8] {
        let out = run_ranks(p, |c| c.allreduce_sum(vals[c.rank()]).expect("sum"));
        let mut want = vals[0];
        for v in &vals[1..p] {
            want += *v;
        }
        for (r, s) in out.iter().enumerate() {
            assert_eq!(s.to_bits(), want.to_bits(), "P={p} rank {r}");
        }
        let mx = run_ranks(p, |c| c.allreduce_max(vals[c.rank()]).expect("max"));
        assert!(mx
            .iter()
            .all(|&m| m == vals[..p].iter().cloned().fold(f64::NEG_INFINITY, f64::max)));
    }
    let x = -0.0f64;
    assert_eq!(
        SerialComm.allreduce_sum(x).expect("sum").to_bits(),
        x.to_bits(),
        "serial returns the local value itself"
    );
    // a missing peer is a located error, not a hang
    let r = run_ranks_timeout();
    assert!(r.contains("rank 0") && r.contains("peer 1"), "{r}");
}

fn run_ranks_timeout() -> String {
    use rcfd_dist::run_ranks_with_timeout;
    let out = run_ranks_with_timeout(2, std::time::Duration::from_millis(200), |c| {
        if c.rank() == 0 {
            c.recv(1, 99).err().map(|e| e.to_string()).unwrap_or_default()
        } else {
            std::thread::sleep(std::time::Duration::from_millis(600));
            String::new()
        }
    });
    out[0].clone()
}
