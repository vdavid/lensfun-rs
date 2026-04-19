#!/usr/bin/env bash
# build.sh — compile probe.cpp against upstream lensfun
# Idempotent: re-running overwrites the binary.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LENSFUN_BUILD="/Users/veszelovszki/projects-git/vdavid/lensfun-rs/related-repos/lensfun/build"
SOURCE="${SCRIPT_DIR}/probe.cpp"
OUTPUT="${SCRIPT_DIR}/probe"

echo "Building probe..."
clang++ -std=c++11 \
  -I"${LENSFUN_BUILD}" \
  -I"${LENSFUN_BUILD}/libs/lensfun" \
  $(pkg-config glib-2.0 --cflags) \
  -L"${LENSFUN_BUILD}/libs/lensfun" \
  -llensfun \
  $(pkg-config glib-2.0 --libs) \
  -Wl,-rpath,"${LENSFUN_BUILD}/libs/lensfun" \
  "${SOURCE}" -o "${OUTPUT}"

echo "Built: ${OUTPUT}"
