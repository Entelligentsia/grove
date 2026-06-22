# Homebrew formula for grove. Lives in the tap repo Entelligentsia/homebrew-grove
# (as Formula/grove.rb), letting users run:
#
#   brew install Entelligentsia/grove/grove
#
# The sha256 values below are per-release placeholders. Regenerate this file
# after tagging with:
#   dist/homebrew/update-formula.sh <vX.Y.Z> > grove.rb
# which fills the version + hashes from the release's .sha256 sidecar assets,
# then copy it into the tap repo as Formula/grove.rb.
class Grove < Formula
  desc "Structural, byte-precise, token-cheap codebase access for coding agents"
  homepage "https://github.com/Entelligentsia/grove"
  version "0.1.3"
  license "MIT"

  BASE = "https://github.com/Entelligentsia/grove/releases/download/v#{version}".freeze

  on_macos do
    on_arm do
      url "#{BASE}/grove-aarch64-apple-darwin.tar.gz"
      sha256 "REPLACE_AARCH64_APPLE_DARWIN"
    end
    on_intel do
      url "#{BASE}/grove-x86_64-apple-darwin.tar.gz"
      sha256 "REPLACE_X86_64_APPLE_DARWIN"
    end
  end

  on_linux do
    on_arm do
      url "#{BASE}/grove-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "REPLACE_AARCH64_UNKNOWN_LINUX_GNU"
    end
    on_intel do
      url "#{BASE}/grove-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "REPLACE_X86_64_UNKNOWN_LINUX_GNU"
    end
  end

  def install
    bin.install "grove"
  end

  test do
    assert_match "grove", shell_output("#{bin}/grove --version")
  end
end
