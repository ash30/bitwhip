let pkgs = import (builtins.fetchTarball https://github.com/NixOS/nixpkgs/archive/nixos-unstable.tar.gz) {
  config.allowUnfree = true; 
  overlays = [ 
    (import (fetchTarball "https://github.com/oxalica/rust-overlay/archive/master.tar.gz"))
  ];
};
  rustc = pkgs.rust-bin.stable.latest.default.override { extensions = ["rust-src"];};
  cargo = pkgs.rust-bin.stable.latest.default;
in
pkgs.mkShell {
  RUST_BACKTRACE = 1;
  inputsFrom = [ (pkgs.callPackage ./default.nix {}) ];

  buildInputs = [
    rustc
    cargo
    pkgs.rust-bin.stable.latest.rust-analyzer # LSP Server
    pkgs.rust-bin.stable.latest.rustfmt       # Formatter
    pkgs.rust-bin.stable.latest.clippy        # Linter
  ];

}


