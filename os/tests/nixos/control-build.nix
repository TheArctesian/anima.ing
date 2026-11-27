let
  workspaceRoot = ../../..;
  flake = builtins.getFlake ("path:" + toString ../..);
  pkgs = import flake.inputs.nixpkgs { system = builtins.currentSystem; };
in {
  sessionAccount = import ./session-account-vm.nix { inherit pkgs workspaceRoot; };
  session = import ./session-vm.nix { inherit pkgs workspaceRoot; };
  package = import ../../packages/control-package.nix { inherit pkgs workspaceRoot; };
  vm = import ./control-vm.nix { inherit pkgs workspaceRoot; };
}
