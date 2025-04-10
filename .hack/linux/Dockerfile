FROM ubuntu:noble@sha256:1e622c5f073b4f6bfad6632f2616c7f59ef256e96fe78bf6a595d1dc4376ac02 AS ubuntu


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

# cache node_modules separately from the source code :)
COPY package.json bun.lock ./
RUN bun i

COPY . .
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
