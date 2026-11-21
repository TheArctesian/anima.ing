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
  options.services.anima-os-updates.package = lib.mkOption {
    type = lib.types.package;
    default = import ./update-package.nix { inherit pkgs workspaceRoot; };
    description = "Source-built Rust update controller with the configured release verification key.";
  };
  config = {
  environment.systemPackages = [ updater ];
  systemd.services.systemd-bless-boot.enable = false;
  systemd.services.anima-update-health = {
    wantedBy = [ "multi-user.target" ];
    after = [ "anima-root-lock.service" "NetworkManager.service" "wpa_supplicant.service" "display-manager.service" "anima-daemon.service" ];
    onFailure = [ "anima-update-failure.service" ];
    serviceConfig = { Type = "oneshot"; TimeoutStartSec = "90s"; ExecStart = "${updater}/bin/anima-os-update health"; };
  };
  systemd.services.anima-update-failure = {
    serviceConfig.Type = "oneshot";
    script = ''${updater}/bin/anima-os-update failure'';
  };
  };
}
