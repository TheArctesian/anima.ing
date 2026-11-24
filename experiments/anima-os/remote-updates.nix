{ lib, updater }: {
  security.sudo.extraRules = [{
    users = [ "admin" ];
    commands = map (action: {
      command = "/run/current-system/sw/bin/anima-os-update remote-${action}";
      options = [ "NOPASSWD" ];
    }) [ "status" "check" "download" "activate" "rollback" ];
  }];
  systemd.services = lib.genAttrs
    (map (action: "anima-update-remote-${action}") [ "check" "download" "activate" "rollback" ])
    (name: {
      description = "Owner-requested remote Anima OS update";
      after = [ "network-online.target" ];
      serviceConfig = {
        Type = "oneshot";
        ExecStart = "${updater}/bin/anima-os-update remote-${lib.removePrefix "anima-update-remote-" name}-run";
        TimeoutStartSec = "2h";
        UMask = "0077";
      };
    });
}
