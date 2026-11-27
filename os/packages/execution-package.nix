{ pkgs, workspaceRoot ? ../.. }:
import ./rust-package.nix {
  inherit pkgs workspaceRoot;
  name = "anima-os-execution";
  testPackages = [ "anima-os-execution" "oci-artifact" ];
  workspaceDependencies = [ "serde" "serde_json" "thiserror" "uuid" "tokio" "sha2" "chrono" "tempfile" "zeroize" "unicode-normalization" "base64" "hmac" "static_assertions" ];
}
