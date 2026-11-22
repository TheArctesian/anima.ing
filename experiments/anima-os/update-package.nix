{ pkgs, workspaceRoot, publicKey ? ./updates/preview-public.pem }:
(import ./rust-package.nix {
  inherit pkgs workspaceRoot;
  name = "anima-os-update";
  workspaceDependencies = [ "base64" "reqwest" "serde" "serde_json" "sha2" "tempfile" "thiserror" "tokio" ];
  extraSourceCommands = ''
    mkdir -p $out/experiments/anima-os/updates
    cp ${publicKey} $out/experiments/anima-os/updates/preview-public.pem
  '';
}).overrideAttrs (old: {
  nativeCheckInputs = (old.nativeCheckInputs or []) ++ [ pkgs.openssl ];
})
