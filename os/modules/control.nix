{ config, lib, pkgs, ... }:
let
  cfg = config.services.anima-os-control;
in {
  options.services.anima-os-control = {
    enable = lib.mkEnableOption "experimental read-only Rust host control";
    publishConsoleSnapshot = lib.mkEnableOption "read-only telemetry for the installed console";
    package = lib.mkOption {
      type = lib.types.package;
      description = "Source-built anima-osd package; explicitly supplied for this opt-in preview.";
    };
  };
  config = lib.mkIf cfg.enable {
    environment.systemPackages = [ cfg.package ];
    users.users.anima-os-control = { isSystemUser = true; group = "anima-os-control"; };
    users.groups.anima-os-control = {};
    users.users.admin = lib.mkIf cfg.publishConsoleSnapshot { extraGroups = [ "anima-os-control" ]; };
    systemd.tmpfiles.rules = [ "d /persist/anima-osd-jobs 0700 anima-os-control anima-os-control -" ]
      ++ lib.optional cfg.publishConsoleSnapshot "d /run/anima-observation 0750 anima-os-control anima-os-control -";
    systemd.services.anima-os-control = {
      description = "Anima OS read-only local host control";
      wantedBy = [ "multi-user.target" ];
      after = [ "local-fs.target" "systemd-tmpfiles-setup.service" ];
      unitConfig.RequiresMountsFor = "/persist";
      serviceConfig = {
        ExecStart = "${cfg.package}/bin/anima-osd serve --state-directory /persist/anima-osd-jobs --systemctl ${pkgs.systemd}/bin/systemctl"
          + lib.optionalString cfg.publishConsoleSnapshot " --console-snapshot /run/anima-observation/console.json";
        User = "anima-os-control";
        Group = "anima-os-control";
        RuntimeDirectory = "anima-osd";
        RuntimeDirectoryMode = "0700";
        UMask = "0077";
        Restart = "on-failure";
        RestartSec = 2;
        NoNewPrivileges = true;
        CapabilityBoundingSet = "";
        ProtectSystem = "strict";
        ReadWritePaths = [ "/persist/anima-osd-jobs" ] ++ lib.optional cfg.publishConsoleSnapshot "/run/anima-observation";
        ProtectHome = true;
        PrivateTmp = true;
        ProtectKernelTunables = true;
        ProtectKernelModules = true;
        ProtectControlGroups = true;
        RestrictAddressFamilies = [ "AF_UNIX" ];
        RestrictSUIDSGID = true;
        LockPersonality = true;
        TasksMax = 32;
        MemoryMax = "64M";
        LimitCORE = 0;
      };
    };
  };
}
