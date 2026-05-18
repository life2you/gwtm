class Gwtm < Formula
  desc "Git worktree manager for local multi-project workflows"
  homepage "https://github.com/life2you/gwtm"
  version "0.1.8"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/life2you/gwtm/releases/download/v0.1.8/gwtm-aarch64-apple-darwin.tar.gz"
      sha256 "79250ae544e1378cbf956871f7dbb187fcfc5d6aa89b0adbefb13d2fd2bc907e"
    end

    on_intel do
      url "https://github.com/life2you/gwtm/releases/download/v0.1.8/gwtm-x86_64-apple-darwin.tar.gz"
      sha256 "9d6da4f765a767c3dfd0c0b6e112146ef97c820238ee5194fb273e3474304b01"
    end
  end

  def install
    bin.install "gwtm"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/gwtm --version")
  end
end
