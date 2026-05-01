class Gwtm < Formula
  desc "Git worktree manager for local multi-project workflows"
  homepage "https://github.com/life2you/gwtm"
  url "https://github.com/life2you/gwtm/archive/refs/tags/v0.1.2.tar.gz"
  sha256 "b2a0378f14d249defbecc8967f81af9ac3ecf1c906146c8b44458547bf1fd193"
  license "MIT"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args(path: ".")
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/gwtm --version")
  end
end
