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
# Application that will run inside the client gets installed here
FROM rust:alpine AS application

RUN apk add --no-cache git

WORKDIR /opt
RUN git clone --branch v2.0.0-rc.2 --depth 1 https://github.com/rvhonorato/gdock
WORKDIR /opt/gdock
RUN cargo build --release

# Binary will be in `/opt/gdock/target/release/gdock`

#===============================================================================
# Layer that will be running the job-orchestrator as `server`
FROM scratch AS server

COPY --from=build /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/
COPY --from=build /opt/target/release/job-orchestrator /job-orchestrator

#===============================================================================
# Layer that will be running the job-orchestrator as `client`
#  it needs to execute in a layer that has `BASH`
FROM alpine:latest AS client

RUN apk add --no-cache bash

COPY --from=build /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/
COPY --from=build /opt/target/release/job-orchestrator /job-orchestrator

# Make sure the example application exists in the `client` layer
COPY --from=application /opt/gdock/target/release/gdock /bin/gdock

#===============================================================================
FROM server AS default
#===============================================================================

