# HausKI base image
FROM mcr.microsoft.com/devcontainers/base:ubuntu-24.04

# Basistools
RUN apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
    ca-certificates curl git build-essential pkg-config \
 && rm -rf /var/lib/apt/lists/*
