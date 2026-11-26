{ pkgs, config, lib, workspaceRoot ? ../.., ... }:
let
  binary = config.services.anima-os-updates.package;
  updater = pkgs.writeShellApplication {
    name = "anima-os-update";
    runtimeInputs = with pkgs; [ openssl xz nix systemd util-linux coreutils ];
    text = ''
      export ANIMA_BLESS_BOOT=${pkgs.systemd}/lib/systemd/systemd-bless-boot
      exec ${binary}/bin/anima-os-update "$@"
    '';
  };
in {
  imports = [ (import ./remote-updates.nix { inherit lib updater; }) ];
  options.services.anima-os-updates.automatic = lib.mkOption {
    type = lib.types.bool;
    default = true;
    description = "Automatically verify and stage updates; activation remains a separate maintenance action.";
  };
  options.services.anima-os-updates.package = lib.mkOption {
    type = lib.types.package;
    default = import ../packages/update-package.nix { inherit pkgs workspaceRoot; };
    description = "Source-built Rust update controller with the configured release verification key.";
  };
  config = {
  environment.systemPackages = [ updater ];
  systemd.timers.anima-update-check = lib.mkIf config.services.anima-os-updates.automatic {
    wantedBy = [ "timers.target" ];
    timerConfig = {
      OnBootSec = "15min";
      OnUnitInactiveSec = "4h";
      RandomizedDelaySec = "15min";
    };
  };
  systemd.services.anima-update-check = lib.mkIf config.services.anima-os-updates.automatic {
    description = "Verify and stage an Anima OS update";
    wants = [ "network-online.target" ];
    after = [ "network-online.target" "anima-update-health.service" ];
    serviceConfig = {
      Type = "oneshot";
      ExecStart = "${updater}/bin/anima-os-update automatic";
      TimeoutStartSec = "2h";
      Nice = 10;
      IOSchedulingClass = "idle";
    };
  };
  systemd.services.systemd-bless-boot.enable = false;
  systemd.timers.anima-update-window = {
    wantedBy = [ "timers.target" ];
    timerConfig = { OnCalendar = "minutely"; AccuracySec = "1s"; };
  };
  systemd.services.anima-update-window = {
    description = "Apply a staged OS update during its approved window";
    after = [ "anima-update-health.service" ];
    unitConfig.ConditionPathExists = "/persist/anima-updates/activation-window.json";
    serviceConfig = {
      Type = "oneshot";
      ExecStart = "${updater}/bin/anima-os-update attempt-window";
      TimeoutStartSec = "3min";
    };
  };
  systemd.services.anima-update-health = {
    wantedBy = [ "multi-user.target" ];
    after = [ "anima-root-lock.service" "anima-execution-recovery.service" "NetworkManager.service" "wpa_supplicant.service" "display-manager.service" "anima-daemon.service" ];
    onFailure = [ "anima-update-failure.service" ];
    serviceConfig = { Type = "oneshot"; TimeoutStartSec = "90s"; ExecStart = "${updater}/bin/anima-os-update health"; };
  };
  systemd.services.anima-update-failure = {
    serviceConfig.Type = "oneshot";
    script = ''${updater}/bin/anima-os-update failure'';
  };
  };
}
