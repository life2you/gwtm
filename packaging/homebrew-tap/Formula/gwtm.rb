class Gwtm < Formula
  desc "Git worktree manager for local multi-project workflows"
  homepage "https://github.com/life2you/gwtm"
  url "https://github.com/life2you/gwtm/archive/refs/tags/v0.1.3.tar.gz"
  sha256 "91cc36dca9ec638f613385282739629cdb99557b488ee079a99ab4d1a18522f4"
  license "MIT"

  depends_on "rust" => :build

  def install
    system "cargo", "install", "--locked", *std_cargo_args(path: ".")
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/gwtm --version")
  end
end
