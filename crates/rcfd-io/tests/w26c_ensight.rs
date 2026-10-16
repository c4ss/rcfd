//! F-103 registered checks X1 and X2 (ADR 0048): the EnSight Gold output is parsed back by a hand-written reader that
//! follows the file layouts independently of the writer code. Third-party readers are not available here.

use rcfd_core::vec3::{self, Vec3};
use rcfd_io::ensight::*;
use rcfd_mesh::gen::{box_mesh, Grading};
use rcfd_mesh::Mesh;

fn file<'a>(f: &'a Files, name: &str) -> &'a [u8] {
    &f.files
        .iter()
        .find(|(n, _)| n == name)
        .unwrap_or_else(|| panic!("missing file {name}"))
        .1
}

struct Geo {
    coords: Vec<Vec3>,
    hexa: Vec<[usize; 8]>,
    nfaced: Vec<Vec<Vec<usize>>>,
}

struct Cur<'a> {
    b: &'a [u8],
    pos: usize,
    ascii: bool,
}

impl<'a> Cur<'a> {
    fn line(&mut self) -> String {
        if self.ascii {
            let e = self.b[self.pos..].iter().position(|c| *c == b'\n').expect("newline");
            let s = String::from_utf8(self.b[self.pos..self.pos + e].to_vec()).unwrap();
            self.pos += e + 1;
            s.trim_end().to_string()
        } else {
            let r = &self.b[self.pos..self.pos + 80];
            self.pos += 80;
            String::from_utf8(r.iter().copied().take_while(|c| *c != 0).collect())
                .unwrap()
                .trim_end()
                .to_string()
        }
    }
    fn int(&mut self) -> usize {
        if self.ascii {
            let l = self.line();
            assert_eq!(l.len(), 10, "I10 line '{l}'");
            l.trim().parse().unwrap()
        } else {
            let v = i32::from_le_bytes(self.b[self.pos..self.pos + 4].try_into().unwrap());
            self.pos += 4;
            v as usize
        }
    }
    fn ints(&mut self, n: usize) -> Vec<usize> {
        if self.ascii {
            let l = self.line();
            assert_eq!(l.len(), 10 * n, "{n} x I10 line '{l}'");
            (0..n).map(|i| l[10 * i..10 * i + 10].trim().parse().unwrap()).collect()
        } else {
            (0..n).map(|_| self.int()).collect()
        }
    }
    fn real(&mut self) -> f64 {
        if self.ascii {
            let l = self.line();
            assert_eq!(l.len(), 12, "E12.5 line '{l}'");
            l.trim().parse().unwrap()
        } else {
            let v = f32::from_le_bytes(self.b[self.pos..self.pos + 4].try_into().unwrap());
            self.pos += 4;
            v as f64
        }
    }
    fn done(&self) -> bool {
        self.pos == self.b.len()
    }
}

fn read_geo(bytes: &[u8], ascii: bool, stem: &str) -> Geo {
    let mut c = Cur {
        b: bytes,
        pos: 0,
        ascii,
    };
    if !ascii {
        assert_eq!(c.line(), "C Binary");
    }
    c.line();
    assert_eq!(c.line(), stem);
    assert_eq!(c.line(), "node id off");
    assert_eq!(c.line(), "element id off");
    assert_eq!(c.line(), "part");
    assert_eq!(c.int(), 1);
    c.line();
    assert_eq!(c.line(), "coordinates");
    let nn = c.int();
    let mut coords = vec![[0.0; 3]; nn];
    for k in 0..3 {
        for p in coords.iter_mut() {
            p[k] = c.real();
        }
    }
    let (mut hexa, mut nfaced) = (Vec::new(), Vec::new());
    while !c.done() {
        match c.line().as_str() {
            "hexa8" => {
                let ne = c.int();
                for _ in 0..ne {
                    let v = c.ints(8);
                    assert!(v.iter().all(|i| *i >= 1 && *i <= nn), "1-based ids");
                    hexa.push(std::array::from_fn(|i| v[i] - 1));
                }
            }
            "nfaced" => {
                let ne = c.int();
                let nf: Vec<usize> = (0..ne).map(|_| c.int()).collect();
                let np: Vec<Vec<usize>> = nf.iter().map(|n| (0..*n).map(|_| c.int()).collect()).collect();
                for faces in np {
                    let mut cell = Vec::new();
                    for n in faces {
                        cell.push(c.ints(n).into_iter().map(|i| i - 1).collect());
                    }
                    nfaced.push(cell);
                }
            }
            other => panic!("unknown element keyword '{other}'"),
        }
    }
    Geo { coords, hexa, nfaced }
}

/// Raw value sequence of a variable file: `(block keyword, values)`.
fn read_var(bytes: &[u8], ascii: bool, name: &str, counts: &[(&str, usize)], ncomp: usize) -> Vec<(String, Vec<f64>)> {
    let mut c = Cur {
        b: bytes,
        pos: 0,
        ascii,
    };
    assert_eq!(c.line(), name);
    assert_eq!(c.line(), "part");
    assert_eq!(c.int(), 1);
    let mut out = Vec::new();
    for (kw, n) in counts {
        assert_eq!(c.line(), *kw);
        out.push((kw.to_string(), (0..n * ncomp).map(|_| c.real()).collect()));
    }
    assert!(c.done(), "trailing bytes");
    out
}

fn close(a: f64, b: f64, rel: f64) -> bool {
    a == b || (a - b).abs() <= rel * a.abs().max(b.abs())
}

fn vol_from_faces(p: &[Vec3], faces: &[Vec<usize>]) -> f64 {
    let mut v = 0.0;
    for f in faces {
        let (c, s) = rcfd_mesh::mesh::polygon_geometry(p, f);
        v += vec3::dot(c, s) / 3.0;
    }
    v
}

const HEX_FACES: [[usize; 4]; 6] = [
    [0, 3, 2, 1],
    [4, 5, 6, 7],
    [0, 1, 5, 4],
    [1, 2, 6, 5],
    [2, 3, 7, 6],
    [3, 0, 4, 7],
];

fn wavy_box() -> Mesh {
    box_mesh(
        [3, 2, 2],
        [1.3, 0.7, 2.1],
        [Grading::Expansion(2.0), Grading::Uniform, Grading::Symmetric(3.0)],
    )
    .unwrap()
}

#[test]
fn e12_matches_c_format() {
    assert_eq!(fmt_e12(1.0), " 1.00000e+00");
    assert_eq!(fmt_e12(-12345.678), "-1.23457e+04");
    assert_eq!(fmt_e12(0.0), " 0.00000e+00");
    assert_eq!(fmt_e12(1.5e-7), " 1.50000e-07");
    assert_eq!(fmt_e12(1e-100), "1.00000e-100");
    assert_eq!(fmt_e12(9.999996), " 1.00000e+01");
}

#[test]
fn x1_round_trip_hex_mesh_ascii_and_binary_transient() {
    let m = wavy_box();
    let nc = m.n_cells();
    let np = m.n_points();
    let times = [0.0, 0.1, 0.25];
    let node_s: Vec<Vec<f64>> = (0..3)
        .map(|s| {
            m.points()
                .iter()
                .map(|p| (p[0] * 3.1 + p[1] - s as f64).sin() * 1e3 + 1e-3)
                .collect()
        })
        .collect();
    let node_v: Vec<Vec<Vec3>> = (0..3)
        .map(|s| {
            m.points()
                .iter()
                .map(|p| [p[0] + s as f64, -p[1] * 1e-5, p[2] * 7.0e4])
                .collect()
        })
        .collect();
    let cell_s: Vec<Vec<f64>> = (0..3)
        .map(|s| (0..nc).map(|c| (c as f64 + 1.0) * 0.37 + s as f64).collect())
        .collect();
    let cell_v: Vec<Vec<Vec3>> = (0..3)
        .map(|s| (0..nc).map(|c| [c as f64, 2.0 * c as f64 + s as f64, -0.5]).collect())
        .collect();
    let steps: Vec<Step> = (0..3)
        .map(|s| Step {
            time: times[s],
            variables: vec![
                Variable {
                    name: "pn",
                    data: Data::NodeScalar(&node_s[s]),
                },
                Variable {
                    name: "un",
                    data: Data::NodeVector(&node_v[s]),
                },
                Variable {
                    name: "pc",
                    data: Data::CellScalar(&cell_s[s]),
                },
                Variable {
                    name: "uc",
                    data: Data::CellVector(&cell_v[s]),
                },
            ],
        })
        .collect();
    assert_eq!(element_counts(&m), (nc, 0));
    for (enc, ascii, rel) in [(Encoding::Ascii, true, 1e-5), (Encoding::Binary, false, 1e-7)] {
        let f = build(&m, "run", enc, &steps).unwrap();
        let case = String::from_utf8(file(&f, "run.case").to_vec()).unwrap();
        for need in [
            "FORMAT\ntype: ensight gold",
            "model: run.geo",
            "scalar per node: 1 pn run.pn.****",
            "vector per node: 1 un run.un.****",
            "scalar per element: 1 pc run.pc.****",
            "vector per element: 1 uc run.uc.****",
            "time set: 1",
            "number of steps: 3",
            "filename start number: 0",
            "filename increment: 1",
            "time values:\n0 0.1 0.25",
        ] {
            assert!(case.contains(need), "case file lacks '{need}':\n{case}");
        }
        let g = read_geo(file(&f, "run.geo"), ascii, "run");
        assert_eq!(g.coords.len(), np);
        for (a, b) in g.coords.iter().zip(m.points()) {
            for k in 0..3 {
                assert!(close(a[k], b[k], rel), "{enc:?} coordinate {} vs {}", a[k], b[k]);
            }
        }
        assert_eq!(g.hexa.len(), nc);
        assert!(g.nfaced.is_empty());
        // each hexa8 reproduces the cell volume with the standard face table (checks node order and handedness)
        for (c, h) in g.hexa.iter().enumerate() {
            let faces: Vec<Vec<usize>> = HEX_FACES.iter().map(|f| f.iter().map(|i| h[*i]).collect()).collect();
            let v = vol_from_faces(m.points(), &faces);
            assert!(
                close(v, m.cell_volumes()[c], 1e-12),
                "cell {c}: {v} vs {}",
                m.cell_volumes()[c]
            );
        }
        if !ascii {
            let expect = 80 * 6 + 4 + 80 * 2 + 4 + 12 * np + 80 + 4 + 32 * nc;
            assert_eq!(file(&f, "run.geo").len(), expect, "binary geometry size");
        }
        for s in 0..3 {
            let fname = |n: &str| format!("run.{n}.{s:04}");
            let tol = |got: &[f64], want: Vec<f64>| {
                assert_eq!(got.len(), want.len());
                for (a, b) in got.iter().zip(want) {
                    assert!(close(*a, b, rel), "{enc:?} step {s}: {a} vs {b}");
                }
            };
            let v = read_var(file(&f, &fname("pn")), ascii, "pn", &[("coordinates", np)], 1);
            tol(&v[0].1, node_s[s].clone());
            let v = read_var(file(&f, &fname("un")), ascii, "un", &[("coordinates", np)], 3);
            tol(
                &v[0].1,
                (0..3).flat_map(|k| node_v[s].iter().map(move |x| x[k])).collect(),
            );
            let v = read_var(file(&f, &fname("pc")), ascii, "pc", &[("hexa8", nc)], 1);
            tol(&v[0].1, cell_s[s].clone());
            let v = read_var(file(&f, &fname("uc")), ascii, "uc", &[("hexa8", nc)], 3);
            tol(
                &v[0].1,
                (0..3).flat_map(|k| cell_v[s].iter().map(move |x| x[k])).collect(),
            );
            if !ascii {
                assert_eq!(file(&f, &fname("pn")).len(), 80 + 80 + 4 + 80 + 4 * np);
                assert_eq!(file(&f, &fname("uc")).len(), 80 + 80 + 4 + 80 + 12 * nc);
            }
        }
        assert_eq!(f.files.len(), 2 + 3 * 4);
    }
}

#[test]
fn x1_static_single_step_has_no_time_section() {
    let m = wavy_box();
    let p = vec![1.0; m.n_cells()];
    let steps = [Step {
        time: 0.0,
        variables: vec![Variable {
            name: "p",
            data: Data::CellScalar(&p),
        }],
    }];
    let f = build(&m, "s", Encoding::Ascii, &steps).unwrap();
    let case = String::from_utf8(file(&f, "s.case").to_vec()).unwrap();
    assert!(!case.contains("TIME") && case.contains("scalar per element: p s.p"));
    assert_eq!(f.files.len(), 3);
}

fn mixed_mesh() -> Mesh {
    let pts: Vec<Vec3> = vec![
        [0., 0., 0.],
        [1., 0., 0.],
        [1., 1., 0.],
        [0., 1., 0.],
        [0., 0., 1.],
        [1., 0., 1.],
        [1., 1., 1.],
        [0., 1., 1.],
        [0., 0., 2.],
        [1., 0., 2.],
        [1., 1., 2.],
        [0., 1., 2.],
        [2., 0., 0.],
        [2., 1., 0.],
    ];
    let a = vec![
        vec![0, 3, 2, 1],
        vec![4, 5, 6, 7],
        vec![0, 4, 7, 3],
        vec![1, 2, 6, 5],
        vec![0, 1, 5, 4],
        vec![3, 7, 6, 2],
    ];
    let prism = vec![
        vec![1, 12, 5],
        vec![2, 6, 13],
        vec![1, 2, 13, 12],
        vec![1, 2, 6, 5],
        vec![12, 13, 6, 5],
    ];
    let b = vec![
        vec![4, 7, 6, 5],
        vec![8, 9, 10, 11],
        vec![4, 8, 11, 7],
        vec![5, 6, 10, 9],
        vec![4, 5, 9, 8],
        vec![7, 11, 10, 6],
    ];
    Mesh::from_cell_faces(pts, &[a, prism, b], &[]).unwrap()
}

#[test]
fn x1_mixed_hex_and_polyhedral_mesh() {
    let m = mixed_mesh();
    assert_eq!(element_counts(&m), (2, 1));
    let cs = [10.0, 20.0, 30.0];
    let cv = [[1.0, 2.0, 3.0], [4.0, 5.0, 6.0], [7.0, 8.0, 9.0]];
    let steps = [Step {
        time: 0.0,
        variables: vec![
            Variable {
                name: "pc",
                data: Data::CellScalar(&cs),
            },
            Variable {
                name: "uc",
                data: Data::CellVector(&cv),
            },
        ],
    }];
    for (enc, ascii, rel) in [(Encoding::Ascii, true, 1e-5), (Encoding::Binary, false, 1e-7)] {
        let f = build(&m, "mix", enc, &steps).unwrap();
        let g = read_geo(file(&f, "mix.geo"), ascii, "mix");
        assert_eq!((g.hexa.len(), g.nfaced.len()), (2, 1));
        for (h, c) in g.hexa.iter().zip([0usize, 2]) {
            let faces: Vec<Vec<usize>> = HEX_FACES.iter().map(|f| f.iter().map(|i| h[*i]).collect()).collect();
            assert!(close(vol_from_faces(&g.coords, &faces), m.cell_volumes()[c], 1e-12));
        }
        // nfaced faces are outward: the divergence-theorem volume is the prism volume 0.5
        let v = vol_from_faces(&g.coords, &g.nfaced[0]);
        assert!(close(v, 0.5, 1e-12), "{v}");
        assert_eq!(g.nfaced[0].len(), 5);
        let want = [
            vec![1usize, 12, 5],
            vec![2, 6, 13],
            vec![1, 2, 13, 12],
            vec![1, 2, 6, 5],
            vec![12, 13, 6, 5],
        ];
        for w in &want {
            let mut ws = w.clone();
            ws.sort_unstable();
            assert!(g.nfaced[0].iter().any(|f| {
                let mut fs = f.clone();
                fs.sort_unstable();
                fs == ws
            }));
        }
        // per-element data in block order: hexa cells 0, 2 first, then the prism (cell 1)
        let v = read_var(file(&f, "mix.pc"), ascii, "pc", &[("hexa8", 2), ("nfaced", 1)], 1);
        let flat: Vec<f64> = v.iter().flat_map(|x| x.1.clone()).collect();
        for (a, b) in flat.iter().zip([10.0, 30.0, 20.0]) {
            assert!(close(*a, b, rel));
        }
        let v = read_var(file(&f, "mix.uc"), ascii, "uc", &[("hexa8", 2), ("nfaced", 1)], 3);
        assert!(close(v[0].1[0], 1.0, rel) && close(v[0].1[1], 7.0, rel) && close(v[0].1[2], 2.0, rel));
        assert!(close(v[1].1[0], 4.0, rel) && close(v[1].1[1], 5.0, rel) && close(v[1].1[2], 6.0, rel));
    }
}

fn st(v: Vec<Variable<'_>>, t: f64) -> Step<'_> {
    Step { time: t, variables: v }
}

#[test]
fn x2_errors_are_located() {
    let m = wavy_box();
    let short = vec![0.0; 3];
    let e = |r: rcfd_core::Result<Files>| r.expect_err("error").to_string();
    let bad_len = [st(
        vec![Variable {
            name: "p",
            data: Data::CellScalar(&short),
        }],
        0.0,
    )];
    assert!(e(build(&m, "x", Encoding::Ascii, &bad_len)).contains("variable 'p' has 3 values, expected 12"));
    let ok = vec![0.0; m.n_cells()];
    let var = |name| Variable {
        name,
        data: Data::CellScalar(&ok),
    };
    let good = [st(vec![var("p")], 0.0)];
    assert!(e(build(&m, "a/b", Encoding::Ascii, &good)).contains("stem 'a/b'"));
    assert!(e(build(&m, "x", Encoding::Ascii, &[])).contains("at least one"));
    let dup = [st(vec![var("p"), var("p")], 0.0)];
    assert!(e(build(&m, "x", Encoding::Ascii, &dup)).contains("duplicate variable name 'p'"));
    let badname = [st(vec![var("p q")], 0.0)];
    assert!(e(build(&m, "x", Encoding::Ascii, &badname)).contains("variable name 'p q'"));
    let times = [st(vec![var("p")], 1.0), st(vec![var("p")], 1.0)];
    assert!(e(build(&m, "x", Encoding::Ascii, &times)).contains("step 1: times"));
    let diff = [st(vec![var("p")], 0.0), st(vec![], 1.0)];
    assert!(e(build(&m, "x", Encoding::Ascii, &diff)).contains("step 1: variables differ"));
    let mut nan = ok.clone();
    nan[2] = f64::NAN;
    let n = [st(
        vec![Variable {
            name: "p",
            data: Data::CellScalar(&nan),
        }],
        0.0,
    )];
    assert!(e(build(&m, "x", Encoding::Ascii, &n)).contains("variable 'p' value 2"));
    let mut big = ok.clone();
    big[1] = 1e300;
    let b = [st(
        vec![Variable {
            name: "p",
            data: Data::CellScalar(&big),
        }],
        0.0,
    )];
    assert!(build(&m, "x", Encoding::Ascii, &b).is_ok());
    assert!(e(build(&m, "x", Encoding::Binary, &b)).contains("single-precision"));
}

#[test]
fn write_creates_the_files_on_disk() {
    let m = wavy_box();
    let p = vec![2.0; m.n_cells()];
    let dir = std::env::temp_dir().join("rcfd_w26c_ensight_write");
    let _ = std::fs::remove_dir_all(&dir);
    let steps = [st(
        vec![Variable {
            name: "p",
            data: Data::CellScalar(&p),
        }],
        0.0,
    )];
    let paths = write(&dir, "w", &m, Encoding::Binary, &steps).unwrap();
    assert_eq!(paths.len(), 3);
    assert!(paths.iter().all(|p| p.exists()));
    assert!(std::fs::read(&paths[1]).unwrap().starts_with(b"C Binary"));
    let _ = std::fs::remove_dir_all(&dir);
}

/// Writes the mixed mesh (ASCII and binary, two steps) to `$RCFD_ENSIGHT_OUT` when set, for the out-of-process check in
/// another language (ADR 0048 addendum); otherwise does nothing.
#[test]
fn export_for_external_check() {
    let Ok(dir) = std::env::var("RCFD_ENSIGHT_OUT") else {
        return;
    };
    let m = mixed_mesh();
    let cs = [[10.0, 20.0, 30.0], [11.0, 21.0, 31.0]];
    let cv = [[1.0, 2.0, 3.0], [4.0, 5.0, 6.0], [7.0, 8.0, 9.0]];
    let nv: Vec<Vec3> = m.points().iter().map(|p| [p[0], 2.0 * p[1], 3.0 * p[2]]).collect();
    let ns: Vec<f64> = m.points().iter().map(|p| p[0] + p[1] + p[2]).collect();
    let steps: Vec<Step> = (0..2)
        .map(|s| {
            st(
                vec![
                    Variable {
                        name: "pc",
                        data: Data::CellScalar(&cs[s]),
                    },
                    Variable {
                        name: "uc",
                        data: Data::CellVector(&cv),
                    },
                    Variable {
                        name: "un",
                        data: Data::NodeVector(&nv),
                    },
                    Variable {
                        name: "pn",
                        data: Data::NodeScalar(&ns),
                    },
                ],
                0.5 * s as f64,
            )
        })
        .collect();
    write(
        std::path::Path::new(&dir).join("ascii"),
        "mix",
        &m,
        Encoding::Ascii,
        &steps,
    )
    .unwrap();
    write(
        std::path::Path::new(&dir).join("binary"),
        "mix",
        &m,
        Encoding::Binary,
        &steps,
    )
    .unwrap();
}
