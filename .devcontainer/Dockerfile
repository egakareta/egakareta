FROM ubuntu:24.04

ENV DEBIAN_FRONTEND=noninteractive

WORKDIR /workspace

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
    libclang-dev

# Install Node.js via nvm and install Node 24 in a single layer
# source nvm in the same RUN so the command is available immediately
RUN curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.40.3/install.sh | bash \
  && export NVM_DIR="$HOME/.nvm" \
  && [ -s "$NVM_DIR/nvm.sh" ] && . "$NVM_DIR/nvm.sh" \
  && nvm install 24 \
  && nvm alias default 24

# Install Bun
RUN curl -fsSL https://bun.sh/install | bash

# Install Rust
RUN bun install:rust
RUN bun install:tools