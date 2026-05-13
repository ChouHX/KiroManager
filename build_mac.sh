#!/bin/bash
# ========================================================================
#  Kiro Manager (Tauri 2) - macOS release 构建
#  需要: Xcode Command Line Tools + Node.js + Rust
# ========================================================================
set -e

cd "$(dirname "$0")"

echo "[1/3] npm install ..."
npm install

echo "[2/3] 构建 CSS (Tailwind) ..."
npx @tailwindcss/cli -i src/input.css -o src/style.css --minify

echo "[3/3] cargo build --release ..."
cd src-tauri
cargo build --release
cd ..

# 复制产物
mkdir -p dist
cp src-tauri/target/release/kiro-manager dist/KiroManager

echo ""
echo "========================================================================"
echo "  构建完成!"
echo "  产物: dist/KiroManager"
echo "========================================================================"
