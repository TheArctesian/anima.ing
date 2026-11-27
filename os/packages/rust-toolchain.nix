{ pkgs, workspaceRoot }:
let
  version = (builtins.fromTOML (builtins.readFile (workspaceRoot + "/rust-toolchain.toml"))).toolchain.channel;
  platform = pkgs.stdenv.hostPlatform.rust.rustcTarget;
  hashes = {
    x86_64-unknown-linux-gnu = "98ee6bec240e1cfe944f17e7092412dc5ef6e008484bb668c6936d35a7d15acd";
    aarch64-unknown-linux-gnu = "a21f6a21818df07c1e2e7addb570af5444b3fa9dd61e3a207f4fb1c5a5e665c9";
  };
  toolchain = pkgs.callPackage (pkgs.path + "/pkgs/development/compilers/rust/binary.nix") {
    inherit version platform;
    versionType = "bootstrap";
    src = pkgs.fetchurl {
      url = "https://static.rust-lang.org/dist/rust-${version}-${platform}.tar.xz";
      sha256 = hashes.${platform} or (throw "AnimaOS has no pinned Rust archive for ${platform}");
    };
  };
in
assert pkgs.lib.assertMsg (version == "1.100.0") "Update AnimaOS Rust archive hashes with rust-toolchain.toml";
pkgs.makeRustPlatform { inherit (toolchain) cargo rustc; }
