{
  description = "AnimaOS UEFI installer and isolated recovery qualification fixture";
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-26.05";
  outputs = { self, nixpkgs }: let
    system = "x86_64-linux";
    pkgs = import nixpkgs { inherit system; };
    workspaceRoot = ./payload/source;
    daemonPackage = import ./packages/daemon-package.nix { inherit pkgs; };
  in {
    packages.${system} = rec {
      console-python = import ./packages/console-python.nix { inherit pkgs; };
      default = iso;
      recovery-image = import ./tests/nixos/image.nix {
        inherit pkgs;
        systemA = self.nixosConfigurations.prototype.config.system.build.toplevel;
        systemB = self.nixosConfigurations.candidate.config.system.build.toplevel;
        paramsA = self.nixosConfigurations.prototype.config.boot.kernelParams;
        paramsB = self.nixosConfigurations.candidate.config.boot.kernelParams;
      };
      iso = self.nixosConfigurations.installer.config.system.build.isoImage;
    };
    nixosConfigurations.appliance = nixpkgs.lib.nixosSystem {
      inherit system;
      specialArgs = { inherit daemonPackage workspaceRoot; };
      modules = [ ./images/appliance.nix ];
    };
    nixosConfigurations.installer = nixpkgs.lib.nixosSystem {
      inherit system;
      specialArgs.workspaceRoot = workspaceRoot;
      specialArgs.installedSystem = self.nixosConfigurations.appliance.config.system.build.toplevel;
      modules = [ ./images/iso.nix ];
    };
    nixosConfigurations.candidate = nixpkgs.lib.nixosSystem {
      inherit system;
      modules = [ ./tests/nixos/machine.nix { anima.revision = "B"; } ];
    };
    nixosConfigurations.prototype = nixpkgs.lib.nixosSystem {
      inherit system;
      modules = [ ./tests/nixos/machine.nix ];
    };
  };
}
