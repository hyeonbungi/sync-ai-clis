# ghcr.io/hyeonbungi/sync-ai-clis — a static musl build of the binary on
# scratch. This image is a COPY --from source for baking sync-ai-clis into
# devcontainer/CI images, not a runtime: scratch has no shell or CA certs,
# which the managed tools' installers need. Typical use:
#
#   COPY --from=ghcr.io/hyeonbungi/sync-ai-clis:latest /sync-ai-clis /usr/local/bin/sync-ai-clis
#   RUN sync-ai-clis --yes --only claude,gemini
#
# (--version/--help do run directly, for smoke tests.)
FROM rust:1-alpine AS builder
RUN apk add --no-cache musl-dev
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release --locked

FROM scratch
ARG VERSION=dev
LABEL org.opencontainers.image.source="https://github.com/hyeonbungi/sync-ai-clis" \
      org.opencontainers.image.description="Detect, install, and keep AI coding CLIs up to date with one command" \
      org.opencontainers.image.licenses="MIT" \
      org.opencontainers.image.version="${VERSION}"
COPY --from=builder /src/target/release/sync-ai-clis /sync-ai-clis
ENTRYPOINT ["/sync-ai-clis"]
