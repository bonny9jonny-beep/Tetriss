# syntax=docker/dockerfile:1

# ───────── Этап 1: Rust → WASM клиент ─────────
FROM rust:1-bookworm AS wasm
WORKDIR /app/client
RUN rustup target add wasm32-unknown-unknown

# wasm-bindgen-cli ОБЯЗАН совпадать по версии с крейтом wasm-bindgen из client/Cargo.lock,
# поэтому версию берём прямо из lock-файла и качаем готовый бинарник (быстрее, чем cargo install).
COPY client/Cargo.toml client/Cargo.lock ./
RUN mkdir src && echo "" > src/lib.rs \
 && cargo build --release --target wasm32-unknown-unknown --locked \
 && rm -rf src
COPY client/src ./src
RUN touch src/lib.rs && cargo build --release --target wasm32-unknown-unknown --locked

RUN set -eux; \
    VER="$(awk -F'"' '/^name = "wasm-bindgen"$/{getline; print $2; exit}' Cargo.lock)"; \
    case "$(uname -m)" in \
      x86_64)  T=x86_64-unknown-linux-musl ;; \
      aarch64) T=aarch64-unknown-linux-gnu ;; \
      *) echo "unsupported arch" >&2; exit 1 ;; \
    esac; \
    curl -fsSL "https://github.com/wasm-bindgen/wasm-bindgen/releases/download/${VER}/wasm-bindgen-${VER}-${T}.tar.gz" \
      | tar xz -C /tmp; \
    /tmp/wasm-bindgen-${VER}-${T}/wasm-bindgen --target web --out-dir /app/pkg \
      target/wasm32-unknown-unknown/release/fsociety_tetris_wasm.wasm

# ───────── Этап 2: Axum-сервер ─────────
FROM rust:1-bookworm AS server
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main(){}" > src/main.rs \
 && cargo build --release --locked && rm -rf src
COPY src ./src
RUN touch src/main.rs && cargo build --release --locked

# ───────── Этап 3: минимальный итоговый образ ─────────
# distroless/cc-debian12 — того же семейства (Debian 12), что и сборочный образ,
# поэтому динамическая линковка с glibc работает.
FROM gcr.io/distroless/cc-debian12:nonroot
COPY --from=server /app/target/release/fsociety-tetris-server /usr/local/bin/server
COPY --from=wasm   /app/pkg /app/pkg
ENV PORT=80 PKG_DIR=/app/pkg
ENTRYPOINT ["/usr/local/bin/server"]
