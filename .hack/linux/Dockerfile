FROM ubuntu:noble@sha256:72297848456d5d37d1262630108ab308d3e9ec7ed1c3286a32fe09856619a782 AS ubuntu


FROM ubuntu AS base

COPY .hack/linux/install-deps.sh .
RUN ./install-deps.sh && apt-get clean && rm -rf /var/lib/apt/lists/*
ENV PKG_CONFIG_PATH="/usr/lib/pkgconfig:/usr/lib/x86_64-linux-gnu/pkgconfig"

RUN curl -fsSL https://bun.sh/install | bash
ENV PATH="/root/.bun/bin:${PATH}"

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
  | sh -s -- -y --profile minimal
ENV PATH="/root/.cargo/bin:${PATH}"


FROM base AS builder

WORKDIR /app/src
COPY . .
RUN bun i
RUN \
    --mount=type=cache,target=/app/src/src-tauri/target/ \
    --mount=type=cache,target=/app/src/src-tauri/bmm-lib/target/ \
    --mount=type=cache,target=/root/.cargo/registry/cache/git/db \
    --mount=type=cache,target=/root/.cargo/registry/cache/registry/ \
  bun tauri build --target x86_64-unknown-linux-gnu --verbose && \
  mkdir /app/bundles && cp \
    src-tauri/target/x86_64-unknown-linux-gnu/release/bundle/deb/*.deb \
    src-tauri/target/x86_64-unknown-linux-gnu/release/bundle/rpm/*.rpm \
    src-tauri/target/x86_64-unknown-linux-gnu/release/bundle/appimage/*.AppImage \
    /app/bundles


FROM ubuntu AS output

COPY --from=builder /app/bundles /app/bundles
