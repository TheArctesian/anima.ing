{ pkgs, workspaceRoot, publicKey ? ../release/signing/preview-public.pem }:
(import ./rust-package.nix {
  inherit pkgs workspaceRoot;
  name = "anima-os-update";
  workspaceDependencies = [ "base64" "reqwest" "serde" "serde_json" "sha2" "tempfile" "thiserror" "tokio" "uuid" ];
  extraSourceCommands = ''
    mkdir -p $out/os/release/signing
    cp ${publicKey} $out/os/release/signing/preview-public.pem
  '';
}).overrideAttrs (old: {
  nativeCheckInputs = (old.nativeCheckInputs or []) ++ [ pkgs.openssl ];
})
