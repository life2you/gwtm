class Gwtm < Formula
  desc "Git worktree manager for local multi-project workflows"
  homepage "https://github.com/life2you/gwtm"
  url "https://github.com/life2you/gwtm/archive/refs/tags/v0.1.5.tar.gz"
  sha256 "d9b3d3e102ec09bef6062786b4039fa753f4776881149cbd0c1ec51035400ae8"
  license "MIT"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args(path: ".")
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/gwtm --version")
  end
end
