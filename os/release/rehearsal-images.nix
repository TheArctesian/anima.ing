{ workspace, osDirectory, updaterFile, fixtureKey }:
let
  flake = builtins.getFlake ("path:" + workspace + "/" + osDirectory);
  source = flake.outPath;
  publicKey = builtins.path {
    name = "anima-rehearsal-public.pem";
    path = /. + fixtureKey;
  };
  fixture = { pkgs, ... }: {
    services.anima-os-updates.package = import (source + "/" + updaterFile) {
      inherit pkgs publicKey;
      workspaceRoot = source + "/payload/source";
    };
    system.systemBuilderCommands = ''
      cp ${publicKey} "$out/anima-rehearsal-public.pem"
    '';
  };
  appliance = flake.nixosConfigurations.appliance.extendModules { modules = [ fixture ]; };
  installedSystem = appliance.config.system.build.toplevel;
  installer = flake.nixosConfigurations.installer.extendModules {
    specialArgs = { inherit installedSystem; };
  };
in {
  system = installedSystem;
  iso = installer.config.system.build.isoImage;
}
