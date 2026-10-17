//! Consistency validator of a mesh decomposition (ADR 0058, D4): every defect is a located error naming the part, the peer and the
//! global cell.

use rcfd_core::{Error, Result};
use rcfd_mesh::mesh::Mesh;
use rcfd_mesh::partition::Decomposition;
use std::collections::BTreeSet;

fn loc(part: usize, peer: usize, cell: usize, what: &str) -> Error {
    Error::Mesh(format!("decomposition: part {part}, peer {peer}, cell {cell}: {what}"))
}

/// Check that `dec` is a consistent decomposition of `mesh`:
/// every cell is owned by exactly one part and `part_of_cell` agrees; the ghosts of a part are exactly the face neighbours of its
/// owned cells that belong to other parts (no missing, no extra, no duplicate), each attributed to the part that owns it; the send
/// and receive lists of every pair of parts have equal length and name the same global cells in the same order.
pub fn validate_decomposition(mesh: &Mesh, dec: &Decomposition) -> Result<()> {
    let n = mesh.n_cells();
    let np = dec.n_parts;
    if dec.part_of_cell.len() != n {
        return Err(Error::Mesh(format!(
            "decomposition: part_of_cell has {} entries for {} cells",
            dec.part_of_cell.len(),
            n
        )));
    }
    if dec.subdomains.len() != np {
        return Err(Error::Mesh(format!(
            "decomposition: {} sub-domains for {} parts",
            dec.subdomains.len(),
            np
        )));
    }
    for (i, s) in dec.subdomains.iter().enumerate() {
        if s.part != i {
            return Err(Error::Mesh(format!(
                "decomposition: sub-domain {i} claims to be part {}",
                s.part
            )));
        }
    }
    let mut adj: Vec<Vec<usize>> = vec![vec![]; n];
    for f in 0..mesh.n_internal_faces() {
        let (a, b) = (mesh.owner()[f], mesh.neighbour()[f]);
        adj[a].push(b);
        adj[b].push(a);
    }
    // ownership
    let mut owner_of = vec![usize::MAX; n];
    for (p, s) in dec.subdomains.iter().enumerate() {
        for &c in &s.owned {
            if c >= n {
                return Err(loc(p, p, c, "owned cell id out of range"));
            }
            if owner_of[c] != usize::MAX {
                return Err(loc(
                    p,
                    owner_of[c],
                    c,
                    &format!("cell is owned by both part {} and part {p} (duplicated)", owner_of[c]),
                ));
            }
            owner_of[c] = p;
        }
    }
    for c in 0..n {
        let claimed = dec.part_of_cell[c];
        if owner_of[c] == usize::MAX {
            let peer = adj[c]
                .iter()
                .map(|&b| dec.part_of_cell[b])
                .find(|&q| q != claimed)
                .unwrap_or(claimed);
            return Err(loc(
                claimed,
                peer,
                c,
                &format!("cell is owned by no part (lost); part_of_cell assigns it to part {claimed}"),
            ));
        }
        if owner_of[c] != claimed {
            return Err(loc(
                owner_of[c],
                claimed,
                c,
                &format!(
                    "part_of_cell assigns the cell to part {claimed} but it is listed as owned by part {}",
                    owner_of[c]
                ),
            ));
        }
    }
    // ghosts
    for (p, s) in dec.subdomains.iter().enumerate() {
        let mut expected: BTreeSet<usize> = BTreeSet::new();
        for &c in &s.owned {
            for &b in &adj[c] {
                if owner_of[b] != p {
                    expected.insert(b);
                }
            }
        }
        let mut seen = BTreeSet::new();
        for &g in &s.ghosts {
            if g >= n {
                return Err(loc(p, p, g, "ghost cell id out of range"));
            }
            if !seen.insert(g) {
                return Err(loc(p, owner_of[g], g, "ghost cell is listed more than once"));
            }
            if owner_of[g] == p {
                return Err(loc(p, p, g, "ghost cell is owned by this part"));
            }
            if !expected.contains(&g) {
                return Err(loc(
                    p,
                    owner_of[g],
                    g,
                    "ghost cell is not a face neighbour of any owned cell (extra ghost)",
                ));
            }
        }
        if let Some(&m) = expected.iter().find(|c| !seen.contains(c)) {
            return Err(loc(
                p,
                owner_of[m],
                m,
                "face neighbour of an owned cell lies in another part but is missing from the ghost list",
            ));
        }
        let mut covered = BTreeSet::new();
        for (&q, ids) in &s.recv {
            if q >= np || q == p {
                return Err(loc(
                    p,
                    q,
                    ids.first()
                        .map_or(0, |&i| s.ghosts.get(i.wrapping_sub(s.n_owned())).copied().unwrap_or(0)),
                    "receive list names an invalid peer",
                ));
            }
            for &id in ids {
                let Some(&g) = id.checked_sub(s.n_owned()).and_then(|k| s.ghosts.get(k)) else {
                    return Err(loc(
                        p,
                        q,
                        0,
                        &format!(
                            "receive local id {id} is not a ghost slot (n_owned {}, n_local {})",
                            s.n_owned(),
                            s.n_local()
                        ),
                    ));
                };
                if owner_of[g] != q {
                    return Err(loc(
                        p,
                        q,
                        g,
                        &format!(
                            "ghost cell is received from peer {q} but is owned by part {}",
                            owner_of[g]
                        ),
                    ));
                }
                covered.insert(g);
            }
        }
        if let Some(&g) = s.ghosts.iter().find(|g| !covered.contains(g)) {
            return Err(loc(p, owner_of[g], g, "ghost cell has no entry in any receive list"));
        }
    }
    // send / receive symmetry
    for (p, s) in dec.subdomains.iter().enumerate() {
        for (&q, sids) in &s.send {
            if q >= np || q == p {
                return Err(loc(p, q, 0, "send list names an invalid peer"));
            }
            let t = &dec.subdomains[q];
            let Some(rids) = t.recv.get(&p) else {
                let c = sids.first().and_then(|&i| s.owned.get(i)).copied().unwrap_or(0);
                return Err(loc(q, p, c, "part sends to a peer that has no receive list for it"));
            };
            if sids.len() != rids.len() {
                let k = sids.len().min(rids.len());
                let c = if sids.len() > rids.len() {
                    s.owned.get(sids[k]).copied().unwrap_or(0)
                } else {
                    rids.get(k)
                        .and_then(|&i| i.checked_sub(t.n_owned()))
                        .and_then(|j| t.ghosts.get(j))
                        .copied()
                        .unwrap_or(0)
                };
                return Err(loc(
                    q,
                    p,
                    c,
                    &format!(
                        "send list of part {p} to peer {q} has {} entries but the receive list has {}",
                        sids.len(),
                        rids.len()
                    ),
                ));
            }
            for (k, (&si, &ri)) in sids.iter().zip(rids).enumerate() {
                let Some(&sg) = s.owned.get(si) else {
                    return Err(loc(
                        p,
                        q,
                        0,
                        &format!("send local id {si} (entry {k}) is not an owned cell"),
                    ));
                };
                let Some(&rg) = ri.checked_sub(t.n_owned()).and_then(|j| t.ghosts.get(j)) else {
                    return Err(loc(
                        q,
                        p,
                        sg,
                        &format!("receive local id {ri} (entry {k}) is not a ghost slot"),
                    ));
                };
                if sg != rg {
                    return Err(loc(
                        q,
                        p,
                        rg,
                        &format!("entry {k} of the message is cell {sg} on the sender but cell {rg} on the receiver"),
                    ));
                }
            }
        }
        for (&q, rids) in &s.recv {
            if !dec.subdomains[q].send.contains_key(&p) {
                let c = rids
                    .first()
                    .and_then(|&i| i.checked_sub(s.n_owned()))
                    .and_then(|j| s.ghosts.get(j))
                    .copied()
                    .unwrap_or(0);
                return Err(loc(p, q, c, "part receives from a peer that has no send list for it"));
            }
        }
    }
    Ok(())
}
