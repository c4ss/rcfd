# The Linux test image of the Electron Studio (F-821, ADR 0801 Addendum 17): Ubuntu 24.04 (the CI runners' distribution,
# glibc 2.39), the gate's Rust toolchain (RUST_GATE_TOOLCHAIN in .github/workflows/gate.yml), Node 22 (package.json
# engines), the libraries Electron 45 needs on Linux, Xvfb for a display, and Mesa's software drivers. There is no GPU in
# the container: Chromium gets no WebGPU adapter and the Studio draws with its WebGL 2 fallback, or not at all where WebGL 2
# is also missing (see the ADR for which specs that touches).
#
# Built and run by apps/studio-web/docker/linux-test/run.mjs; the repository's files are not part of the image (the build
# context is this folder only).
FROM ubuntu:24.04
ARG RUST_TOOLCHAIN=1.99.0
ARG NODE_VERSION=22.19.0
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
       ca-certificates curl xz-utils build-essential pkg-config python3 git procps \
       xvfb xauth dbus dbus-x11 fonts-liberation fonts-dejavu-core \
       libgtk-3-0t64 libnss3 libnspr4 libasound2t64 libgbm1 libxss1 libxtst6 libatk-bridge2.0-0t64 libatk1.0-0t64 \
       libcups2t64 libdrm2 libxkbcommon0 libxcomposite1 libxdamage1 libxrandr2 libxfixes3 libpango-1.0-0 libcairo2 \
       libsecret-1-0 libnotify4 libuuid1 \
       libgl1 libegl1 libgles2 libgl1-mesa-dri mesa-vulkan-drivers libvulkan1 \
       dpkg-dev fakeroot rpm file \
    && rm -rf /var/lib/apt/lists/*
ENV RUSTUP_HOME=/usr/local/rustup CARGO_HOME=/usr/local/cargo PATH=/usr/local/cargo/bin:/usr/local/node/bin:$PATH
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
    | sh -s -- -y --profile minimal --default-toolchain "$RUST_TOOLCHAIN" \
    && rustc --version && cargo --version
RUN mkdir -p /usr/local/node \
    && curl --proto '=https' --tlsv1.2 -sSfL "https://nodejs.org/dist/v${NODE_VERSION}/node-v${NODE_VERSION}-linux-x64.tar.xz" \
       | tar -xJ -C /usr/local/node --strip-components=1 \
    && node --version && npm --version
# Electron's renderers run in Chromium's sandbox (G10), which root cannot use: the tests run as this user. The sandbox
# needs user namespaces, which Docker's default seccomp profile refuses, so run.mjs starts the container with
# --security-opt seccomp=unconfined (the container is looser; the app's sandbox is the one a desktop gets).
RUN useradd --create-home --uid 1001 studio
# The steps themselves (inside.sh) come with the source of each run, so the image only changes with this file.
