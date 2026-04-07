/*

 * Copyright (c) egakareta <team@egakareta.com>.
 * Licensed under the GNU AGPLv3 or a proprietary Commercial License.
 * See LICENSE and COMMERICAL.md for details.

 */
FROM ubuntu:24.04

ENV DEBIAN_FRONTEND=noninteractive

# Install necessary packages
RUN apt-get update \
  && apt-get install -y --no-install-recommends \
    build-essential \
    curl \
    ca-certificates \
    git \
    git-lfs \
    pkg-config \
    libssl-dev \
    cmake \
    gnupg \
    wget \
    unzip \
    libclang-dev \
    libasound2-dev \
    libasound2-plugins \
    libmariadb-dev \
    libudev-dev \
    libxkbcommon-dev \
    libxkbcommon-x11-dev \
    libwayland-dev \
    libx11-dev \
    libxcursor-dev \
    libxi-dev \
    libxrandr-dev \
    libvulkan-dev \
    libfontconfig1-dev \
    mesa-vulkan-drivers \
    libgl1-mesa-dri \
    libgl1 \
    xvfb \
    alsa-utils \
    x11-utils \
    pkg-config \
    gcc \
    swig \
    pipewire-alsa \
    pipewire

# Install Node.js via nvm and install Node 24 in a single layer
# source nvm in the same RUN so the command is available immediately
RUN curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.40.3/install.sh | bash \
  && export NVM_DIR="$HOME/.nvm" \
  && [ -s "$NVM_DIR/nvm.sh" ] && . "$NVM_DIR/nvm.sh" \
  && nvm install 24 \
  && nvm alias default 24

# Install Bun
RUN curl -fsSL https://bun.sh/install | bash
ENV PATH="/root/.bun/bin:${PATH}"

# Install Rust
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain nightly
ENV PATH="/root/.cargo/bin:${PATH}"

# Install cargo tools
RUN curl -L --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh | bash

RUN rustup component add clippy llvm-tools-preview rustfmt rust-src
RUN cargo binstall cargo-llvm-cov -y
