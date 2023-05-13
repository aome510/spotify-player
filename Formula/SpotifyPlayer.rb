class SpotifyPlayer < Formula
  desc "Cross-platform Text Expander written in Rust"
  homepage "https://github.com/aome510/spotify-player"
  url "https://github.com/slano-ls/homebrew-spotify-player/releases/latest/download/spotify-player-mac.tar.gz"
  sha256 "f0685151774566835bfe94bb68c8bb77517ad54eda32936e7364681463f748a5"
  version "0.1.0"

  def install
    bin.install "SpotifyPlayer"
  end
end
