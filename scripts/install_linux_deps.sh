#!/usr/bin/env bash
set -euxo pipefail

# fix: unable to compile libspa, related issue: https://github.com/pop-os/cosmic-epoch/issues/280
sudo add-apt-repository ppa:pipewire-debian/pipewire-upstream

# Disable interactive prompts (e.g. tzdata)
export DEBIAN_FRONTEND=noninteractive

# Install all required system dependencies for Linux build (from .devcontainer/Dockerfile)
sudo apt-get update
sudo apt-get install -y \
    curl \
    git \
    python3-pip \
    build-essential \
    pkg-config \
    libgbm-dev \
    libegl1-mesa-dev \
    libwayland-dev \
    libxkbcommon-dev \
    libudev-dev \
    libdbus-1-dev \
    libssl-dev \
    libxi-dev \
    libxtst-dev \
    libpipewire-0.3-dev \
    libclang-dev \
    clang \
    tzdata \
    ca-certificates 