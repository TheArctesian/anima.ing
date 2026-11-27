{ pkgs, workspaceRoot ? ../.. }:
import ./rust-package.nix {
  inherit pkgs workspaceRoot;
  name = "anima-os-maintenance";
  workspaceDependencies = [ "serde" "serde_json" "thiserror" "uuid" "tokio" ];
}
