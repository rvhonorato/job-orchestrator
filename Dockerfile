#===============================================================================
# Main stage that will build the job-orchestrator
FROM rust:alpine AS build

RUN apk add --no-cache \
  musl-dev \
  build-base \
  curl \
  ca-certificates \
  pkgconfig

WORKDIR /opt
COPY . .

RUN cargo build --release

#===============================================================================
# Example application - replace this with your own application.
#
#  `gdock` is used here as a demonstration; any application can be
#  used instead, as long as it is callable from a `run.sh` script.
#
#  This stage compiles a binary from source. Alternatives include:
#   - Copying a pre-built binary from a URL or local path
#   - Installing a Python package (pip install ...)
#   - Any other setup, as long as the result is available for
#     COPY --from=application in the client stage below
FROM rust:alpine AS application

RUN apk add --no-cache git

WORKDIR /opt
RUN git clone --branch v2.0.0-rc.2 --depth 1 https://github.com/rvhonorato/gdock
WORKDIR /opt/gdock
RUN cargo build --release

#===============================================================================
# Runtime base - includes bash (required for client mode to
#  execute run.sh scripts) and the job-orchestrator binary.
#
#  This is the image published to ghcr.io; it can be used as
#  either server or client:
#   docker run <image> /job-orchestrator server
#   docker run <image> /job-orchestrator client
FROM alpine:3.23.3 AS runtime

# TODO: Run as non-root user for production hardening. Requires migrating
# existing volume ownership first (see README Security section).
#   addgroup -S appgroup && adduser -S appuser -G appgroup -u 10001
RUN apk add --no-cache bash

COPY --from=build /opt/target/release/job-orchestrator /job-orchestrator

#===============================================================================
# Server target - runtime only, no application needed
FROM runtime AS server

#===============================================================================
# Client target - extends runtime with the example application.
#
#  Replace the COPY below with your own application binary or
#  install your own dependencies here. The client executes jobs
#  by running `bash run.sh` in the payload directory, so any
#  tool referenced in run.sh must be available in this image.
FROM runtime AS client

COPY --from=application /opt/gdock/target/release/gdock /bin/gdock

#===============================================================================
FROM runtime AS default
#===============================================================================
