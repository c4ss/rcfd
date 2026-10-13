# ADR 0003: Parallel strategy

Status: accepted.

Decision: shared-memory parallelism with `rayon` first (cell and face loops
chunked deterministically; reductions use a fixed chunking and fixed combine
order so results are bit-reproducible for a given thread count and chunk size).
Distributed memory (MPI) comes later (F-100) on top of the partitioning and
halo structures of F-016; kernels are written against owned+ghost cell
ranges so the same code runs in both modes. GPU (F-101) targets only linear
solves and flux assembly behind a trait.
