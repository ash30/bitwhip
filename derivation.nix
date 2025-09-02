{ lib
, rustPlatform
, pkg-config
, cmake
, openssl
, SDL2
, ffmpeg-full
, stdenv
}:

rustPlatform.buildRustPackage rec {
  pname = "bitwhip";
  version = "0.1.0";

  src = ./.;

  cargoLock = {
    lockFile = ./Cargo.lock;
  };

  nativeBuildInputs = [
    pkg-config
    cmake
    rustPlatform.bindgenHook
    openssl
  ];

  buildInputs = [
    openssl
    ffmpeg-full
  ];

  # Skip tests that require display/audio devices
  doCheck = false;

  meta = with lib; {
    description = "WebRTC streaming application implementing WHIP/WHEP protocols";
    longDescription = ''
      BitWHIP is a WebRTC streaming application written in Rust that implements 
      WHIP (WebRTC HTTP Ingestion Protocol) and WHEP (WebRTC HTTP Egress Protocol) 
      for real-time video streaming. 
    '';
    homepage = "https://github.com/bitwhip/bitwhip";
    license = licenses.mit; # Adjust based on actual license
    maintainers = [ ]; # Add maintainer if desired
    platforms = platforms.unix;
    
    # Binary names that will be installed
    mainProgram = "bitwhip";
  };
}
