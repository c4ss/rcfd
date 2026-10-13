# ADR 0002: Field storage

Status: accepted.

Decision: fields are flat `Vec<f64>` (structure of arrays) indexed by cell id
for cell values and by face id for face values; boundary faces are stored as the
tail of the face range (faces `n_internal..n_faces`), grouped by patch, as in
the owner/neighbour convention. Vector fields are `Vec<[f64; 3]>`. Scalar
kernels that must support f32 are generic over `rcfd_core::Scalar`; solver
state defaults to `Real = f64`. Meshes are immutable after construction; fields
reference a mesh by `Arc`.

Consequences: contiguous memory, trivial rayon chunking, cheap halo layers
(ghost cells appended after owned cells).
