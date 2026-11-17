#!/data/data/com.termux/files/usr/bin/bash
set -e

echo "[1/3] Сборка WASM..."
cargo build --manifest-path client/Cargo.toml --target wasm32-unknown-unknown --release

echo "[2/3] Генерация JS/WASM бандла..."
wasm-bindgen --target web --out-dir pkg client/target/wasm32-unknown-unknown/release/fsociety_tetris_wasm.wasm

echo "[3/3] Запуск Axum сервера..."
cargo run --release
