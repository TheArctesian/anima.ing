{ pkgs, workspaceRoot }:
let
  inherit (pkgs) lib;
  manifest = builtins.fromTOML (builtins.readFile (workspaceRoot + "/Cargo.toml"));
  lock = builtins.fromTOML (builtins.readFile (workspaceRoot + "/Cargo.lock"));
  resolve = reference:
    let
      parts = lib.splitString " " reference;
      matches = builtins.filter (package:
        package.name == builtins.head parts &&
        (builtins.length parts == 1 || package.version == builtins.elemAt parts 1)
      ) lock.package;
    in assert builtins.length matches == 1; builtins.head matches;
  closure = builtins.genericClosure {
    startSet = [ { key = "anima-osd"; package = resolve "anima-osd"; } ];
    operator = entry: map (dependency:
      let package = resolve dependency;
      in { key = package.name + " " + package.version; inherit package; }
    ) (entry.package.dependencies or []);
  };
  toml = pkgs.formats.toml {};
  packageLock = toml.generate "anima-osd-Cargo.lock" {
    inherit (lock) version;
    package = map (entry: entry.package) closure;
  };
  workspace = toml.generate "anima-osd-workspace.toml" {
    workspace = {
      inherit (manifest.workspace) resolver package lints;
      members = [ "crates/anima-osd" ];
      dependencies = lib.getAttrs [ "clap" "serde" "serde_json" "thiserror" "tokio" "tempfile" "uuid" ] manifest.workspace.dependencies;
    };
  };
  source = pkgs.runCommand "anima-osd-source" {} ''
    mkdir -p $out/crates
    cp -r ${workspaceRoot + "/crates/anima-osd"} $out/crates/anima-osd
    cp ${workspace} $out/Cargo.toml
    cp ${packageLock} $out/Cargo.lock
  '';
in pkgs.rustPlatform.buildRustPackage {
  pname = "anima-osd";
  version = manifest.workspace.package.version;
  src = source;
  cargoLock.lockFile = packageLock;
  cargoBuildFlags = [ "-p" "anima-osd" ];
  cargoTestFlags = [ "-p" "anima-osd" ];
  meta.platforms = lib.platforms.linux;
}
