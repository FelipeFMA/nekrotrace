#!/bin/bash
set -e

# Ensure we are in the project root
cd "$(dirname "$0")/.."

echo "Building Frontend..."
npm run build

echo "Building Server..."
cargo build --release -p nekrotrace-server

echo "Packaging..."
rm -rf dist
mkdir -p dist
cp target/release/nekrotrace-server dist/
cp -r build dist/

echo "Done! Run ./dist/nekrotrace-server to start the server."
