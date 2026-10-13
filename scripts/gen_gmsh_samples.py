import gmsh, os, sys
out = sys.argv[1]
os.makedirs(out, exist_ok=True)
print("gmsh", gmsh.__version__)

def build(kind):
    gmsh.initialize()
    gmsh.option.setNumber("General.Terminal", 0)
    gmsh.model.add(kind)
    if kind == "tet":
        gmsh.model.occ.addBox(0, 0, 0, 2, 1, 1)
        gmsh.model.occ.synchronize()
        gmsh.option.setNumber("Mesh.MeshSizeMax", 0.5)
    elif kind == "hex":
        gmsh.model.occ.addBox(0, 0, 0, 2, 1, 1)
        gmsh.model.occ.synchronize()
        for c in gmsh.model.getEntities(1):
            gmsh.model.mesh.setTransfiniteCurve(c[1], 5 if True else 3)
        for s in gmsh.model.getEntities(2):
            gmsh.model.mesh.setTransfiniteSurface(s[1])
            gmsh.model.mesh.setRecombine(2, s[1])
        gmsh.model.mesh.setTransfiniteVolume(1)
    elif kind == "prism":
        gmsh.model.occ.addRectangle(0, 0, 0, 2, 1)
        gmsh.model.occ.synchronize()
        gmsh.option.setNumber("Mesh.MeshSizeMax", 0.5)
        gmsh.model.occ.extrude([(2, 1)], 0, 0, 1, numElements=[3], recombine=True)
        gmsh.model.occ.synchronize()
    # physical groups: by face normal side
    surfs = gmsh.model.getEntities(2)
    left, right, other = [], [], []
    for _, t in surfs:
        com = gmsh.model.occ.getCenterOfMass(2, t)
        if abs(com[0]) < 1e-9:
            left.append(t)
        elif abs(com[0] - 2) < 1e-9:
            right.append(t)
        else:
            other.append(t)
    gmsh.model.addPhysicalGroup(2, left, 1, "inlet")
    gmsh.model.addPhysicalGroup(2, right, 2, "outlet")
    gmsh.model.addPhysicalGroup(2, other, 3, "walls")
    gmsh.model.addPhysicalGroup(3, [t for _, t in gmsh.model.getEntities(3)], 7, "fluid")
    if kind == "prism":
        # extruded rectangle leaves the base surface 1 as a boundary too: it has no group assigned above
        pass
    gmsh.model.mesh.generate(3)
    for ver in ("2.2", "4.1"):
        for binary in (False, True):
            gmsh.option.setNumber("Mesh.MshFileVersion", float(ver))
            gmsh.option.setNumber("Mesh.Binary", 1 if binary else 0)
            gmsh.write(os.path.join(out, f"{kind}_v{ver}_{'bin' if binary else 'ascii'}.msh"))
    # stats
    types, tags, nodes = gmsh.model.mesh.getElements(3)
    stats = {int(t): len(e) for t, e in zip(types, tags)}
    print(kind, "3D element counts by gmsh type:", stats)
    gmsh.finalize()

for k in ("tet", "hex", "prism"):
    build(k)
