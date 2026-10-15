//! ADR 0025 checks R1-R3 (F-073): pixel values at known field points, the roadmap timing criterion
//! (1 M cells, under 2 s) and PNG round trip. CPU rasteriser only; no claim about how images look.

use rcfd_mesh::gen::cavity_2d_rect;
use rcfd_post::render::{colormap, render_scalar, RenderOptions};
use std::time::Instant;

fn lcg(s: &mut u64) -> f64 {
    *s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    (*s >> 11) as f64 / (1u64 << 53) as f64
}

#[test]
fn r1_pixels_at_known_cells_equal_the_colour_map() {
    let n = 64usize;
    let mesh = cavity_2d_rect(n, n, 1.0, 1.0).unwrap();
    // Cell values are unique pseudo-random numbers so that the cell-to-pixel mapping is really tested, not a gradient.
    let centres = mesh.cell_centres();
    let mut s = 5u64;
    let vals: Vec<f64> = (0..mesh.n_cells()).map(|_| lcg(&mut s)).collect();
    let opt = RenderOptions {
        width: 640,
        colorbar: false,
        ..Default::default()
    };
    let r = render_scalar(&mesh, &vals, &opt).unwrap();
    let (vmin, vmax) = (r.vmin, r.vmax);
    // Stated geometry of the renderer: 8 px margin, uniform scale (width - 16)/L, image row 0 at y = ymax.
    let (margin, scale) = (8.0, (640.0 - 16.0) / 1.0);
    let mut ok = 0;
    let mut probes = 0;
    for k in 0..20 {
        let c = (lcg(&mut s) * mesh.n_cells() as f64) as usize % mesh.n_cells();
        let (cx, cy) = (centres[c][0], centres[c][1]);
        let px = (margin + cx * scale).round() as usize;
        let py = (margin + (1.0 - cy) * scale).round() as usize;
        let want = colormap((vals[c] - vmin) / (vmax - vmin));
        let got = r.image.pixel(px, py);
        probes += 1;
        let good = (0..3).all(|j| (i32::from(got[j]) - i32::from(want[j])).abs() <= 1);
        if !good {
            eprintln!("probe {k}: cell {c} at ({cx:.4},{cy:.4}) pixel ({px},{py}) got {got:?} want {want:?}");
        }
        ok += usize::from(good);
    }
    assert_eq!((ok, probes), (20, 20));
}

#[test]
fn r3_png_round_trip() {
    let mesh = cavity_2d_rect(32, 32, 1.0, 1.0).unwrap();
    let vals: Vec<f64> = mesh.cell_centres().iter().map(|c| c[0] * 3.0 - c[1]).collect();
    let r = render_scalar(&mesh, &vals, &RenderOptions::default()).unwrap();
    let bytes = r.image.to_png().unwrap();
    let dec = png::Decoder::new(std::io::Cursor::new(&bytes));
    let mut rd = dec.read_info().unwrap();
    let mut buf = vec![0; rd.output_buffer_size().unwrap()];
    let info = rd.next_frame(&mut buf).unwrap();
    assert_eq!(
        (info.width as usize, info.height as usize),
        (r.image.width, r.image.height)
    );
    let channels = buf.len() / (r.image.width * r.image.height);
    let mut diff = 0;
    for i in 0..r.image.width * r.image.height {
        if buf[i * channels..i * channels + 3] != r.image.rgba[i * 4..i * 4 + 3] {
            diff += 1;
        }
    }
    assert_eq!(diff, 0);
}

/// The roadmap criterion: a contour image of a 1 M cell mesh in under 2 s. A release-build figure; ignored in debug builds.
#[test]
#[cfg_attr(debug_assertions, ignore = "timing criterion is for release builds")]
fn r2_one_million_cells_render_in_under_two_seconds() {
    let n = 1000usize;
    let mesh = cavity_2d_rect(n, n, 1.0, 1.0).unwrap();
    let vals: Vec<f64> = mesh.cell_centres().iter().map(|c| (6.0 * c[0]).sin() * c[1]).collect();
    assert_eq!(mesh.n_cells(), 1_000_000);
    let opt = RenderOptions {
        width: 1024,
        ..Default::default()
    };
    let t0 = Instant::now();
    let r = render_scalar(&mesh, &vals, &opt).unwrap();
    let render = t0.elapsed();
    let t1 = Instant::now();
    let png = r.image.to_png().unwrap();
    let encode = t1.elapsed();
    eprintln!(
        "r2: 1M-cell render {:.3} s, PNG encode {:.3} s ({} bytes), cells drawn {}",
        render.as_secs_f64(),
        encode.as_secs_f64(),
        png.len(),
        r.cells_drawn
    );
    assert_eq!(r.cells_drawn, 1_000_000);
    assert!(render.as_secs_f64() < 2.0, "render took {render:?}");
}
