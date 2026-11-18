#!/usr/bin/env bash
# Локальная сборка и запуск без Docker.
# Нужно: rustup target add wasm32-unknown-unknown ; cargo install wasm-bindgen-cli --version <как в client/Cargo.lock>
set -euo pipefail
[ -f .env ] && set -a && . ./.env && set +a

echo "[1/3] Сборка WASM-клиента..."
cargo build --manifest-path client/Cargo.toml --target wasm32-unknown-unknown --release

echo "[2/3] Генерация glue-кода wasm-bindgen..."
wasm-bindgen --target web --out-dir pkg \
  client/target/wasm32-unknown-unknown/release/fsociety_tetris_wasm.wasm

echo "[3/3] Запуск Axum-сервера на :${PORT:-3000}..."
cargo run --release
