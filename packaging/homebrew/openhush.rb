class Openhush < Formula
  desc "Open-source voice-to-text that acts as a seamless whisper keyboard"
  homepage "https://github.com/claymore666/openhush"
  url "https://github.com/claymore666/openhush/archive/refs/tags/v0.5.0.tar.gz"
  sha256 "PLACEHOLDER_SHA256"  # Update with actual SHA256 on release
  license "MIT"
  head "https://github.com/claymore666/openhush.git", branch: "main"

  depends_on "rust" => :build
  depends_on "cmake" => :build

  # macOS system dependencies
  on_macos do
    depends_on xcode: :build
  end

  def install
    # Build with Metal support on macOS for GPU acceleration
    if OS.mac?
      system "cargo", "build", "--release", "--features", "metal"
    else
      system "cargo", "build", "--release"
    end

    bin.install "target/release/openhush"

    # Install shell completions if available
    # bash_completion.install "completions/openhush.bash" => "openhush"
    # zsh_completion.install "completions/openhush.zsh" => "_openhush"
    # fish_completion.install "completions/openhush.fish"
  end

  def caveats
    <<~EOS
      OpenHush requires Accessibility permissions to simulate keyboard input.

      To grant permissions:
      1. Open System Preferences > Privacy & Security > Accessibility
      2. Add and enable "openhush" or your terminal app

      Quick start:
        # Download a Whisper model
        openhush model download small

        # Start the daemon
        openhush start

        # Hold Right Ctrl and speak!

      For more information, visit:
        https://github.com/claymore666/openhush/wiki
    EOS
  end

  test do
    assert_match "openhush", shell_output("#{bin}/openhush --version")
  end
end
