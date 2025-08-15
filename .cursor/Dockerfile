# Base on Ubuntu 22.04 LTS for stability and wide package support.
FROM ubuntu:22.04

# Prevent interactive prompts during package installation.
ENV DEBIAN_FRONTEND=noninteractive

# Install all necessary system dependencies from both original Dockerfiles.
# This includes build tools, Python, and various libraries for UI automation and graphics on Linux.
RUN apt-get update && apt-get install -y \
    build-essential \
    ca-certificates \
    clang \
    curl \
    git \
    gnupg \
    libclang-dev \
    libdbus-1-dev \
    libdbus-glib-1-2 \
    libegl1-mesa-dev \
    libpipewire-0.3-dev \
    libssl-dev \
    libudev-dev \
    libwayland-dev \
    libxdo-dev \
    libxkbcommon-dev \
    libxtst-dev \
    pkg-config \
    python3-pip \
    tzdata \
    # Clean up apt cache to keep the image size down.
    && rm -rf /var/lib/apt/lists/*

# Install Node.js (LTS version) using the official NodeSource repository.
# This is more reliable for Docker builds than using nvm.
RUN mkdir -p /etc/apt/keyrings && \
    curl -fsSL https://deb.nodesource.com/gpgkey/nodesource-repo.gpg.key | gpg --dearmor -o /etc/apt/keyrings/nodesource.gpg && \
    echo "deb [signed-by=/etc/apt/keyrings/nodesource.gpg] https://deb.nodesource.com/node_20.x nodistro main" | tee /etc/apt/sources.list.d/nodesource.list && \
    apt-get update && \
    apt-get install -y nodejs && \
    rm -rf /var/lib/apt/lists/*

# Create a non-root user 'agent' for better security.
RUN useradd -m -s /bin/bash agent

# Switch to the 'agent' user.
USER agent

# Set the working directory to the agent's home directory.
WORKDIR /home/agent

# Install the Rust toolchain using rustup as the 'agent' user.
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y

# Add Rust's cargo bin directory to the PATH for this and all future sessions.
ENV PATH="/home/agent/.cargo/bin:${PATH}"

# Set a default command to start a bash shell.
CMD ["/bin/bash"]
