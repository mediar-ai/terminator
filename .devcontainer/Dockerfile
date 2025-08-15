FROM ubuntu:22.04

# Disable interactive prompts (e.g. tzdata)
ENV DEBIAN_FRONTEND=noninteractive

# Install system dependencies
RUN apt update && apt install -y \
    curl \
    git \
    python3-pip \
    build-essential \
    pkg-config \
    libegl1-mesa-dev \
    libwayland-dev \
    libxkbcommon-dev \
    libudev-dev \
    libdbus-1-dev \
    libssl-dev \
    libpipewire-0.3-dev \
    libclang-dev \
    clang \
    tzdata \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Install Rust via rustup
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y

# Add Rust to PATH
ENV PATH="/root/.cargo/bin:${PATH}"

# Default command
CMD ["/bin/bash"] 