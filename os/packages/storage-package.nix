{ pkgs, workspaceRoot }:
let
  binary = import ./rust-package.nix {
    inherit pkgs workspaceRoot;
    name = "anima-os-storage";
    workspaceDependencies = [ "serde" "serde_json" "thiserror" "uuid" ];
  };
in pkgs.writeShellApplication {
  name = "anima-os-storage";
  runtimeInputs = [ pkgs.coreutils pkgs.util-linux ];
  text = ''
    exec ${binary}/bin/anima-os-storage "$@"
  '';
}
