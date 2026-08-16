# Homebrew Cask for OpenHush DMG installer
# Use this if distributing as a .app bundle

cask "openhush" do
  version "0.8.0"
  sha256 "52882913ac51b26d1414db247022ad73b228c9bc32a070a5e6e08c11f7f5db25"

  # Filename must match what release.yml actually uploads:
  # openhush-v<version>-macos-universal.dmg
  url "https://github.com/claymore666/openhush/releases/download/v#{version}/openhush-v#{version}-macos-universal.dmg"
  name "OpenHush"
  desc "Open-source voice-to-text that acts as a seamless whisper keyboard"
  homepage "https://github.com/claymore666/openhush"

  livecheck do
    url :url
    strategy :github_latest
  end

  # Requires Accessibility permissions
  accessibility_access true

  app "OpenHush.app"
  binary "#{appdir}/OpenHush.app/Contents/MacOS/openhush"

  postflight do
    system_command "#{appdir}/OpenHush.app/Contents/MacOS/openhush",
                   args: ["model", "download", "small"],
                   sudo: false
  end

  zap trash: [
    "~/Library/Application Support/openhush",
    "~/Library/Preferences/org.openhush.plist",
    "~/Library/Caches/openhush",
    "~/.config/openhush",
  ]

  caveats <<~EOS
    OpenHush requires Accessibility permissions to simulate keyboard input.

    After installation:
    1. Open System Preferences > Privacy & Security > Accessibility
    2. Enable "OpenHush"

    The default Whisper model (small) will be downloaded automatically.
  EOS
end
