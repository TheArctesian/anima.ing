{ pkgs, ... }:
let
  identity = pkgs.writeShellApplication {
    name = "anima-vnc-identity";
    runtimeInputs = [ pkgs.openssl pkgs.coreutils pkgs.util-linux ];
    text = builtins.readFile ./vnc-identity.sh;
  };
in {
  systemd.tmpfiles.rules = [ "d /persist/vnc 0700 root root -" ];
  systemd.services.anima-vnc-identity = {
    description = "Persistent private VNC server identity";
    after = [ "systemd-tmpfiles-setup.service" ];
    unitConfig.RequiresMountsFor = "/persist";
    serviceConfig = {
      Type = "oneshot";
      RemainAfterExit = true;
      ExecStart = "${identity}/bin/anima-vnc-identity";
      UMask = "0077";
      ProtectSystem = "strict";
      ProtectHome = true;
      ReadWritePaths = [ "/persist/vnc" ];
      PrivateTmp = true;
      NoNewPrivileges = true;
    };
  };
}
