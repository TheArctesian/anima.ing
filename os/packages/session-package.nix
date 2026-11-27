{ pkgs, workspaceRoot ? ../.. }:
import ./rust-package.nix {
  inherit pkgs workspaceRoot;
  name = "anima-os-session";
  workspaceDependencies = [ "serde" "serde_json" "thiserror" "tokio" "reqwest" "uuid" "zeroize" "tempfile" "whoami" "base64" ];
  testPackages = [ "anima-os-session" "remote-login" ];
}
