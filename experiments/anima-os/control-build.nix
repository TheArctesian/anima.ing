let
  workspaceRoot = ../..;
  flake = builtins.getFlake ("path:" + toString ./.);
  pkgs = import flake.inputs.nixpkgs { system = builtins.currentSystem; };
in {
  package = import ./control-package.nix { inherit pkgs workspaceRoot; };
  vm = import ./control-vm.nix { inherit pkgs workspaceRoot; };
}
