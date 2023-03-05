#!/bin/sh
npm i
npm run uno
cargo install trunk
rustup target add wasm32-unknown-unknown
TRUNK_BUILD_PUBLIC_URL="/magic-sql-gen/" trunk build --release
