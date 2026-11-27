# Existing isolated controller tests; no artifact signing or publication.
let
  workspaceRoot = ./..;
  flake = builtins.getFlake ("path:" + toString ./.);
  pkgs = import flake.inputs.nixpkgs { system = builtins.currentSystem; };
in (import ./tests/nixos/control-build.nix) // {
  applications = import ./packages/applications-package.nix { inherit pkgs workspaceRoot; };
  execution = import ./packages/execution-package.nix { inherit pkgs workspaceRoot; };
  maintenance = import ./packages/maintenance-package.nix { inherit pkgs workspaceRoot; };
  storage = import ./packages/storage-package.nix { inherit pkgs workspaceRoot; };
  install = import ./tests/nixos/installer-vm.nix { inherit pkgs workspaceRoot; };
  update = import ./tests/nixos/update-rust-vm.nix { inherit pkgs workspaceRoot; };
  boot = import ./tests/nixos/update-boot-vm.nix { inherit pkgs workspaceRoot; };
}
