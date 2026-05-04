#!/usr/bin/env bash
# scripts/self_check.sh
# Run CodeMetrics against its own codebase with real coverage data.
# Usage: ./scripts/self_check.sh [--text]
set -euo pipefail

FORMAT="${1:-json}"

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LCOV_PATH="$REPO_ROOT/target/lcov.info"

echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "  CodeMetrics Self-Check"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

# Step 1: Generate coverage data
echo ""
echo "▶ Generating LLVM coverage data..."
cargo llvm-cov \
  --workspace \
  --lcov \
  --output-path "$LCOV_PATH" \
  --exclude codemetrics-server \
  --exclude mutation-test \
  --quiet
echo "  ✓ Coverage written to target/lcov.info"

# Step 2: Build the CLI
echo ""
echo "▶ Building codemetrics CLI..."
cargo build -p codemetrics-cli --quiet
echo "  ✓ Built"

# Step 3: Run all checks
echo ""
echo "▶ Running codemetrics self-check..."
"$REPO_ROOT/target/debug/codemetrics" check "$REPO_ROOT" \
  --recursive \
  --format text \
  --coverage "$LCOV_PATH" \
  --max-crap 70 \
  --min-doc 50 \
  --max-debt 20 \
  --max-complexity-violations 80 \
  --max-duplication 20 \
  --max-risk 100.0 \
  --max-fuzz-risk 200

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
