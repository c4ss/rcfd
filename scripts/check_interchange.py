#!/usr/bin/env python3
"""Third-party reader check of the F-119 interchange writers (ADR 0011). Manual, not part of CI.

Needs `pip install vtk meshio numpy` and a built rcfd binary. Usage:
    python scripts/check_interchange.py <rcfd-binary> <work-dir> [plot3d-sample.x]
Every line printed starts with PASS, FAIL or SKIP. Exit status 1 if any FAIL.
"""
import os
import subprocess
import sys

import vtk
from vtk.util.numpy_support import vtk_to_numpy

HERE = os.path.dirname(os.path.abspath(__file__))
DATA = os.path.join(os.path.dirname(HERE), "crates", "rcfd-io", "tests", "data")
fails = 0


def report(ok, msg):
    global fails
    print(("PASS " if ok else "FAIL ") + msg)
    if not ok:
        fails += 1


def convert(rcfd, src, dst, *extra):
    r = subprocess.run([rcfd, "convert", src, dst, *extra], capture_output=True, text=True)
    if r.returncode != 0:
        report(False, f"convert {os.path.basename(src)} -> {os.path.basename(dst)}: {r.stderr.strip()}")
        return False
    return True


def dims_of(b):
    e = b.GetExtent()
    return (e[1] - e[0] + 1, e[3] - e[2] + 1, e[5] - e[4] + 1)


def cell_volume_sum(grid):
    f = vtk.vtkCellSizeFilter()
    f.SetInputData(grid)
    f.ComputeVolumeOn()
    f.ComputeSumOff()
    f.Update()
    vol = vtk_to_numpy(f.GetOutput().GetCellData().GetArray("Volume"))
    return float(vol.sum()), float(vol.min())


def main():
    rcfd, work = sys.argv[1], sys.argv[2]
    os.makedirs(work, exist_ok=True)
    expect = {"hex": 64, "tet": 1139, "prism": 336}
    for kind, ncell in expect.items():
        src = os.path.join(DATA, f"{kind}_v4.1_ascii.msh")
        # STL
        for mode, extra in (("binary", []), ("ascii", ["--ascii"])):
            dst = os.path.join(work, f"{kind}_{mode}.stl")
            if convert(rcfd, src, dst, *extra):
                r = vtk.vtkSTLReader()
                r.SetFileName(dst)
                r.Update()
                poly = r.GetOutput()
                tris = poly.GetNumberOfCells()
                m = vtk.vtkMassProperties()
                m.SetInputData(poly)
                m.Update()
                report(tris > 0 and abs(m.GetVolume() - 2.0) < 1e-4,
                       f"vtkSTLReader {kind} {mode}: {tris} triangles, enclosed volume {m.GetVolume():.6f} (box volume 2)")
        # Tecplot
        dst = os.path.join(work, f"{kind}.dat")
        if convert(rcfd, src, dst):
            r = vtk.vtkTecplotReader()
            r.SetFileName(dst)
            try:
                r.Update()
                mb = r.GetOutput()
                blk = mb.GetBlock(0)
                n = blk.GetNumberOfCells()
                report(n == ncell, f"vtkTecplotReader {kind}: {n} cells (expected {ncell})")
                if n == ncell:
                    s, mn = cell_volume_sum(blk)
                    report(abs(s - 2.0) < 1e-9 and mn > 0, f"vtkTecplotReader {kind}: volume sum {s:.9f}, min cell volume {mn:.3e}")
            except Exception as e:  # noqa
                report(False, f"vtkTecplotReader {kind}: {e}")
        # XDMF
        dst = os.path.join(work, f"{kind}.xmf")
        if convert(rcfd, src, dst):
            r = vtk.vtkXdmfReader()
            r.SetFileName(dst)
            try:
                r.Update()
                out = r.GetOutputDataObject(0)
                grid = out
                while isinstance(grid, vtk.vtkMultiBlockDataSet) or isinstance(grid, vtk.vtkMultiPieceDataSet):
                    grid = grid.GetBlock(0) if isinstance(grid, vtk.vtkMultiBlockDataSet) else grid.GetPiece(0)
                n = grid.GetNumberOfCells()
                ok = n == ncell
                if ok:
                    s, mn = cell_volume_sum(grid)
                    report(abs(s - 2.0) < 1e-9 and mn > 0, f"vtkXdmfReader {kind}: {n} cells, volume sum {s:.9f}, min cell volume {mn:.3e}")
                else:
                    report(False, f"vtkXdmfReader {kind}: {n} cells (expected {ncell})")
            except Exception as e:  # noqa
                report(False, f"vtkXdmfReader {kind}: {e}")
    # Mixed-shape file set written by the Rust test (RCFD_EXTERNAL_DIR=<dir>/mixed cargo test -p rcfd-io --test interchange)
    mixed = os.path.join(work, "mixed")
    if os.path.isdir(mixed):
        total, ncell, ntri = open(os.path.join(mixed, "mixed.volume")).read().split()
        total, ncell, ntri = float(total), int(ncell), int(ntri)
        r = vtk.vtkTecplotReader()
        r.SetFileName(os.path.join(mixed, "mixed.dat"))
        r.Update()
        g = r.GetOutput().GetBlock(0)
        s_, mn = cell_volume_sum(g)
        report(g.GetNumberOfCells() == ncell and abs(s_ - total) < 1e-7 and mn > 0,
               f"vtkTecplotReader mixed (reader stores float32 coordinates, tolerance 1e-7) (hex, pyramid, tet, prism): {g.GetNumberOfCells()} cells, volume {s_:.12f} vs {total:.12f}, min {mn:.3e}")
        arr = g.GetCellData().GetArray("p")
        report(arr is not None and [arr.GetValue(i) for i in range(4)] == [10.0, 11.0, 12.0, 13.0],
               "vtkTecplotReader mixed: cell data p = 10..13 as written")
        r = vtk.vtkXdmfReader()
        r.SetFileName(os.path.join(mixed, "mixed.xmf"))
        r.Update()
        g = r.GetOutputDataObject(0)
        while isinstance(g, vtk.vtkMultiBlockDataSet):
            g = g.GetBlock(0)
        s_, mn = cell_volume_sum(g)
        report(g.GetNumberOfCells() == ncell and abs(s_ - total) < 1e-12 and mn > 0,
               f"vtkXdmfReader mixed: {g.GetNumberOfCells()} cells, volume {s_:.12f} vs {total:.12f}, min {mn:.3e}")
        arr = g.GetCellData().GetArray("p")
        report(arr is not None and [arr.GetValue(i) for i in range(4)] == [10.0, 11.0, 12.0, 13.0],
               "vtkXdmfReader mixed: cell data p = 10..13 as written")
        r = vtk.vtkSTLReader()
        r.SetFileName(os.path.join(mixed, "mixed_patch.stl"))
        r.Update()
        report(r.GetOutput().GetNumberOfCells() == ntri, f"vtkSTLReader mixed patch: {r.GetOutput().GetNumberOfCells()} triangles (written {ntri})")
        for name, binary in (("sample_bin.x", 1), ("sample_asc.x", 0)):
            r = vtk.vtkMultiBlockPLOT3DReader()
            r.SetFileName(os.path.join(mixed, name))
            r.SetBinaryFile(binary)
            r.SetMultiGrid(1)
            r.SetHasByteCount(1 if binary else 0)
            r.SetDoublePrecision(1)
            r.SetAutoDetectFormat(0)
            r.SetIBlanking(0)
            r.Update()
            mb = r.GetOutput()
            dims = [dims_of(mb.GetBlock(i)) for i in range(mb.GetNumberOfBlocks())]
            report(dims == [(5, 4, 3), (2, 2, 2), (7, 3, 4)], f"vtkMultiBlockPLOT3DReader {name}: block dims {dims}")
    else:
        print("SKIP mixed-shape set not present")
    if len(sys.argv) > 3:
        r = vtk.vtkMultiBlockPLOT3DReader()
        r.SetFileName(sys.argv[3])
        r.SetBinaryFile(1)
        r.SetMultiGrid(0)
        r.SetHasByteCount(0)
        r.SetDoublePrecision(0)
        r.SetByteOrderToBigEndian()
        r.SetAutoDetectFormat(0)
        r.Update()
        mb = r.GetOutput()
        dims = [dims_of(mb.GetBlock(i)) for i in range(mb.GetNumberOfBlocks())]
        print("INFO vtkMultiBlockPLOT3DReader on", os.path.basename(sys.argv[3]), "block dims", dims)
        b = mb.GetBlock(0)
        print("INFO bounds", b.GetBounds())
    else:
        print("SKIP plot3d public sample not given")
    # GLB loaded by VTK's glTF reader (a third-party loader, not a viewer): files from
    # RCFD_EXTERNAL_DIR=<work>/glb cargo test -p rcfd-post --test glb_check write_glb
    glb = os.path.join(work, "glb", "contour_zmax.glb")
    if os.path.exists(glb):
        r = vtk.vtkGLTFReader()
        r.SetFileName(glb)
        r.Update()
        blk = r.GetOutput().GetBlock(0)
        while isinstance(blk, vtk.vtkMultiBlockDataSet):
            blk = blk.GetBlock(0)
        names = [blk.GetPointData().GetArrayName(i) for i in range(blk.GetPointData().GetNumberOfArrays())]
        report(blk.GetNumberOfPoints() == 1600 and blk.GetNumberOfCells() == 800 and "COLOR_0" in names and "_SCALAR" in names,
               f"vtkGLTFReader contour_zmax.glb: {blk.GetNumberOfPoints()} points, {blk.GetNumberOfCells()} triangles, arrays {names}")
    else:
        print("SKIP glb files not present")
    sys.exit(1 if fails else 0)


main()
