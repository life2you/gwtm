#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
FORMULA_PATH="$REPO_ROOT/packaging/homebrew-tap/Formula/gwtm.rb"

OWNER="${OWNER:-life2you}"
REPO="${REPO:-gwtm}"
VERSION="${1:-$(sed -n 's/^version = "\(.*\)"/\1/p' "$REPO_ROOT/Cargo.toml" | head -n1)}"

if [[ -z "$VERSION" ]]; then
  echo "Failed to detect version from Cargo.toml" >&2
  exit 1
fi

TAG="v$VERSION"
URL="https://github.com/$OWNER/$REPO/archive/refs/tags/$TAG.tar.gz"
SHA256="$(curl -fsSL "$URL" | shasum -a 256 | awk '{print $1}')"

cat > "$FORMULA_PATH" <<EOF
class Gwtm < Formula
  desc "Git worktree manager for local multi-project workflows"
  homepage "https://github.com/$OWNER/$REPO"
  url "$URL"
  sha256 "$SHA256"
  license "MIT"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args(path: ".")
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/gwtm --version")
  end
end
EOF

echo "Updated $FORMULA_PATH"
echo "Version: $VERSION"
echo "SHA256:  $SHA256"
