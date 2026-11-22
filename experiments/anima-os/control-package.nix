{ pkgs, workspaceRoot }:
import ./rust-package.nix {
  inherit pkgs workspaceRoot;
  name = "anima-osd";
  workspaceDependencies = [ "clap" "serde" "serde_json" "thiserror" "tokio" "tempfile" "uuid" ];
}
