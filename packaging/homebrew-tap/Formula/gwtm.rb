class Gwtm < Formula
  desc "Git worktree manager for local multi-project workflows"
  homepage "https://github.com/life2you/gwtm"
  version "0.1.8"
  url "https://github.com/life2you/gwtm/archive/refs/tags/v0.1.8.tar.gz"
  sha256 "ed6d8849952e02df72a22216165e00c6e3cda3458f76b215206b6b011edcc13e"
  license "MIT"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args(path: ".")
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/gwtm --version")
  end
end
