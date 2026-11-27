{ pkgs, workspaceRoot }:
let
  binary = import ./rust-package.nix {
    inherit pkgs workspaceRoot;
    name = "anima-os-install";
    workspaceDependencies = [ "clap" "serde" "serde_json" "thiserror" "uuid" "zeroize" "tempfile" ];
  };
in pkgs.writeShellApplication {
  name = "anima-os-install";
  runtimeInputs = with pkgs; [ util-linux parted btrfs-progs dosfstools systemd nixos-install-tools nix coreutils ];
  text = ''
    exec ${binary}/bin/anima-os-install "$@"
  '';
}
