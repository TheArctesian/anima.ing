{ pkgs, workspaceRoot, ... }:
let
  storage = import ../packages/storage-package.nix { inherit pkgs workspaceRoot; };
in {
  environment.systemPackages = [ storage ];
  systemd.services.anima-data-volumes = {
    description = "Mount recorded AnimaOS data volumes";
    wantedBy = [ "multi-user.target" ];
    after = [ "local-fs.target" ];
    unitConfig.RequiresMountsFor = "/persist";
    serviceConfig = {
      Type = "oneshot";
      RemainAfterExit = true;
      ExecStart = "${storage}/bin/anima-os-storage";
    };
  };
}
