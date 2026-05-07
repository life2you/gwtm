class Gwtm < Formula
  desc "Git worktree manager for local multi-project workflows"
  homepage "https://github.com/life2you/gwtm"
  url "https://github.com/life2you/gwtm/archive/refs/tags/v0.1.6.tar.gz"
  sha256 "cbed880710d7eff86e1df57218e96b6d33fb21a215d18f288b91bf639ef98978"
  license "MIT"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args(path: ".")
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/gwtm --version")
  end
end
