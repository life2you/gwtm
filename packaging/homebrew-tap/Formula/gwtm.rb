class Gwtm < Formula
  desc "Git worktree manager for local multi-project workflows"
  homepage "https://github.com/life2you/gwtm"
  version "0.1.8"
  url "https://github.com/life2you/gwtm/archive/refs/tags/v0.1.8.tar.gz"
  sha256 "6e83881779d606f20098a8a097af39c5c46adef0509790213b167c43bb0d23e3"
  license "MIT"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args(path: ".")
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/gwtm --version")
  end
end
