let pkgs = import (builtins.fetchTarball {
  name = "nixpkgs-unstable";
  url = "https://github.com/nixos/nixpkgs/archive/035f8c0853c2977b24ffc4d0a42c74f00b182cd8.tar.gz";
  # Hash obtained using `nix-prefetch-url --unpack <url>`
  sha256 = "10mkjpj3wigr6w5azrq0nf784kncf6pplm075ndniakhbwkwjwb2";
}) {
  config.allowUnfree = true; 
  overlays = [ 
    (import (fetchTarball "https://github.com/oxalica/rust-overlay/archive/master.tar.gz"))
  ];
};
  rustc = pkgs.rust-bin.stable.latest.default.override { extensions = ["rust-src"];};
  cargo = pkgs.rust-bin.stable.latest.default;
  rustPlatform = pkgs.makeRustPlatform {
    rustc = rustc;
    cargo = cargo;
  };
in
pkgs.mkShell {
  nativeBuildInputs = [
    rustPlatform.bindgenHook
    rustc
    cargo
    pkgs.cmake
    pkgs.pkg-config
  ];
  buildInputs = [
    pkgs.openssl
    pkgs.SDL2
    pkgs.ffmpeg-full
    pkgs.rust-bin.stable.latest.rust-analyzer # LSP Server
    pkgs.rust-bin.stable.latest.rustfmt       # Formatter
    pkgs.rust-bin.stable.latest.clippy        # Linter
  ];
  RUST_SRC_PATH = "${rustc}/lib/rustlib/src/rust/library/";
    shellHook = ''
    export BINDGEN_EXTRA_CLANG_ARGS="$BINDGEN_EXTRA_CLANG_ARGS $(pkg-config --cflags libswscale)"
    #export FFMPEG_DIR=${pkgs.ffmpeg-full.lib}

    CONFIG_FILE=".cargo/config.toml"
    S="PKG_CONFIG = { value =\"${pkgs.pkg-config}/bin/pkg-config\", relative = false, force = true }" 

    if grep -q "^PKG_CONFIG =" "$CONFIG_FILE"; then
      sed -i "s|^PKG_CONFIG =.*|$S|" ".cargo/config.toml"
    else
      echo $S >> "$CONFIG_FILE"
    fi
    
    # Clear out hardcoded 
    sed -i "s|^FFMPEG_DIR =.*||" ".cargo/config.toml"
    sed -i "s|^VCPKG_ROOT =.*||" ".cargo/config.toml"
  '';

}


