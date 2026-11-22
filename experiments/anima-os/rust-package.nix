{ pkgs, workspaceRoot, name, workspaceDependencies, extraSourceCommands ? "" }:
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
    startSet = [ { key = name; package = resolve name; } ];
    operator = entry: map (dependency:
      let package = resolve dependency;
      in { key = package.name + " " + package.version; inherit package; }
    ) (entry.package.dependencies or []);
  };
  toml = pkgs.formats.toml {};
  packageLock = toml.generate "${name}-Cargo.lock" {
    inherit (lock) version;
    package = map (entry: entry.package) closure;
  };
  workspace = toml.generate "${name}-workspace.toml" {
    workspace = {
      inherit (manifest.workspace) resolver package lints;
      members = [ "crates/${name}" ];
      dependencies = lib.getAttrs workspaceDependencies manifest.workspace.dependencies;
    };
  };
  source = pkgs.runCommand "${name}-source" {} ''
    mkdir -p $out/crates
    cp -r ${workspaceRoot + "/crates/${name}"} $out/crates/${name}
    cp ${workspace} $out/Cargo.toml
    cp ${packageLock} $out/Cargo.lock
    ${extraSourceCommands}
  '';
in pkgs.rustPlatform.buildRustPackage {
  pname = name;
  version = manifest.workspace.package.version;
  src = source;
  cargoLock.lockFile = packageLock;
  cargoBuildFlags = [ "-p" name ];
  cargoTestFlags = [ "-p" name ];
  meta.platforms = lib.platforms.linux;
}
