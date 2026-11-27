{ lib, updater }: {
  security.sudo.extraRules = [{
    users = [ "admin" ];
    commands = (map (action: {
      command = "/run/current-system/sw/bin/anima-os-update remote-${action}";
      options = [ "NOPASSWD" ];
    }) [ "status" "check" "download" ]) ++ (map (action: {
      command = "/run/current-system/sw/bin/anima-os-update ^remote-${action} --review [a-f0-9]{64}$";
      options = [ "NOPASSWD" ];
    }) [ "activate" "rollback" ]);
  }];
  systemd.services = lib.genAttrs
    (map (action: "anima-update-remote-${action}") [ "check" "download" "activate@" "rollback@" ])
    (name: let
      reviewed = lib.hasSuffix "@" name;
      action = lib.removeSuffix "@" (lib.removePrefix "anima-update-remote-" name);
      needsNetwork = builtins.elem action [ "check" "download" ];
    in {
      description = "Owner-requested remote Anima OS update";
      wants = lib.optional needsNetwork "network-online.target";
      after = lib.optional needsNetwork "network-online.target";
      serviceConfig = {
        Type = "oneshot";
        ExecStart = "${updater}/bin/anima-os-update remote-${action}-run"
          + lib.optionalString reviewed " --review %i";
        TimeoutStartSec = "2h";
        UMask = "0077";
      };
    });
}
