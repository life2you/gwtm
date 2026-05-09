class Gwtm < Formula
  desc "Git worktree manager for local multi-project workflows"
  homepage "https://github.com/life2you/gwtm"
  version "0.1.6"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/life2you/gwtm/releases/download/v0.1.6/gwtm-aarch64-apple-darwin.tar.gz"
      sha256 "REPLACE_WITH_ARM64_SHA256"
    end

    on_intel do
      url "https://github.com/life2you/gwtm/releases/download/v0.1.6/gwtm-x86_64-apple-darwin.tar.gz"
      sha256 "REPLACE_WITH_X64_SHA256"
    end
  end

  def install
    bin.install "gwtm"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/gwtm --version")
  end
end
