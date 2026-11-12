# Obraz do budowania Millwise na Linuksa: Ubuntu 22.04 (glibc 2.35), żeby
# program działał także na starszych systemach (Debian 12+, Ubuntu 22.04+).
FROM ubuntu:22.04
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update \
 && apt-get install -y --no-install-recommends build-essential curl ca-certificates pkg-config file \
 && rm -rf /var/lib/apt/lists/*
ARG RUST_VERSION=1.99.1
ENV RUSTUP_HOME=/opt/rustup CARGO_HOME=/opt/cargo PATH=/opt/cargo/bin:$PATH
RUN curl -fsSL https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain "$RUST_VERSION" \
 && chmod -R a+rX /opt/rustup /opt/cargo
