#!/bin/bash
set -e

# Ensure we are in the project root
cd "$(dirname "$0")/.."

echo "Building Frontend..."
npm run build

echo "Ensuring musl target is installed..."
rustup target add x86_64-unknown-linux-musl

echo "Building Server (Static with MUSL)..."
# Build with musl to create a static binary that runs on any Linux distro
cargo build --release --target x86_64-unknown-linux-musl -p nekrotrace-server

echo "Packaging..."
rm -rf dist
mkdir -p dist
# Copy from the musl target directory
cp target/x86_64-unknown-linux-musl/release/nekrotrace-server dist/
cp -r build dist/

echo "Done! The binary in ./dist/nekrotrace-server is now statically linked."
