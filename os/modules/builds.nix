{ pkgs, ... }:
let
  railpack = import ../packages/railpack-package.nix { inherit pkgs; };
in {
  imports = [ ./build-sandbox.nix ];
  environment.systemPackages = [ railpack pkgs.buildkit ];
  environment.etc."anima/build-toolchain.json".text = builtins.toJSON {
    schema = 1;
    railpackVersion = railpack.version;
    railpackFrontend = railpack.frontend;
    buildkitVersion = pkgs.buildkit.version;
    strategies = [ "railpack" "dockerfile" "prebuilt" ];
    output = "oci";
    automaticDeployment = false;
  };
}
