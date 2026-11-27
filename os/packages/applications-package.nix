{ pkgs, workspaceRoot ? ../.. }:
import ./rust-package.nix {
  inherit pkgs workspaceRoot;
  name = "anima-applications";
  workspaceDependencies = [ "getrandom" "serde" "serde_json" "sha2" "tempfile" "thiserror" "tokio" "uuid" "zeroize" ];
}
