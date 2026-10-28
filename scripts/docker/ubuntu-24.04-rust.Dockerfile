# Linux x86_64 build environment of example-projects-linux/ (ADR 0150): the distribution of the CI runners
# (`ubuntu-latest` = Ubuntu 24.04, glibc 2.39) with the gate's Rust toolchain (RUST_GATE_TOOLCHAIN in gate.yml).
# Mesh generators call the C library's libm, whose last bits differ per platform (ADR 0137), so the Linux stores are
# built where the Linux gate runs:
#
#   docker build -t rcfd-linux:ubuntu24.04-rust1.99.0 -f scripts/docker/ubuntu-24.04-rust.Dockerfile scripts/docker
#
# scripts/make_example_projects.py --docker builds this image when it is missing.
FROM ubuntu:24.04
ARG RUST_TOOLCHAIN=1.99.0
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl build-essential pkg-config python3 \
    && rm -rf /var/lib/apt/lists/*
ENV RUSTUP_HOME=/usr/local/rustup CARGO_HOME=/usr/local/cargo PATH=/usr/local/cargo/bin:$PATH
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
    | sh -s -- -y --profile minimal --default-toolchain "$RUST_TOOLCHAIN" --component clippy --component rustfmt \
    && rustc --version && cargo --version
