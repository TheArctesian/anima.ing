{ pkgs, workspaceRoot, ... }:
let
  applications = import ../packages/applications-package.nix { inherit pkgs workspaceRoot; };
  applicationRoot = "/persist/anima-applications";
  applicationSettings = "${applicationRoot}/settings.json";
  applicationClient = "${applicationRoot}/docker-client";
  dockerConfiguration = ''
    configuration=/etc/anima/docker-client
    if [ -e ${applicationClient}/config.json ] || [ -L ${applicationClient}/config.json ]; then
      configuration=${applicationClient}
    fi
  '';
  serveApplications = pkgs.writeShellScript "anima-applications-serve" ''
    ${dockerConfiguration}
    exec ${applications}/bin/anima-applications serve ${applicationSettings} "$configuration" ${applicationRoot} /run/anima-applications
  '';
  enableHosting = pkgs.writeShellApplication {
    name = "anima-enable-hosting";
    runtimeInputs = [ pkgs.systemd ];
    text = ''
      if [ "$#" -ne 1 ] || [ "$1" != --enable ]; then
        echo "Usage: sudo anima-enable-hosting --enable" >&2
        echo "Allows the enrolled Hub to manage Anima applications on this node's system Docker engine." >&2
        exit 2
      fi
      ${applications}/bin/anima-hosting-settings --enable ${applicationRoot} \
        /run/current-system/sw/bin/docker /run/current-system/sw/bin/docker-compose
      if ! systemctl start anima-applications.service; then
        echo "Hosting settings were saved. After resolving the service error, run sudo systemctl start anima-applications.service." >&2
        exit 1
      fi
      echo "Hosting settings saved. Check anima-applications.service and the node's Hosting status in Anima."
    '';
  };
  applicationTerminal = pkgs.writeShellScriptBin "anima-application-terminal" ''
    set -eu
    [ "$#" -eq 2 ] || exit 2
    case "$1" in
      target) operation=console-target ;;
      open) operation=console-terminal ;;
      *) exit 2 ;;
    esac
    ${dockerConfiguration}
    exec ${applications}/bin/anima-applications "$operation" ${applicationSettings} "$configuration" ${applicationRoot} "$2"
  '';
in
{
  system.systemBuilderCommands = ''
    printf '1\n' > "$out/anima-hosting-cutover"
    printf '2\n' > "$out/anima-hosting-cutover-captured"
  '';
  virtualisation.docker = {
    enable = true;
    enableOnBoot = true;
    autoPrune.enable = false;
    daemon.settings = {
      live-restore = true;
      log-driver = "local";
      log-opts = { max-size = "10m"; max-file = "3"; };
    };
  };
  # Retain the legacy engine and its data until each workload has been migrated.
  virtualisation.podman = { enable = true; dockerCompat = false; };
  systemd.tmpfiles.rules = [
    "d /run/anima-os 0700 root root -"
    "d ${applicationRoot} 0700 root root -"
    "d ${applicationRoot}/imports 0700 root root -"
    "d ${applicationClient} 0700 root root -"
    "d /persist/anima-admission 0700 root root -"
    "d /persist/anima-admission/reservations 0700 root root -"
  ];
  environment.systemPackages = [
    pkgs.docker-compose
    pkgs.podman-compose
    pkgs.util-linux
    applications
    applicationTerminal
    enableHosting
  ];
  environment.etc."anima/docker-client/config.json" = {
    text = "{}";
    mode = "0600";
  };
  systemd.services.anima-applications = {
    description = "AnimaOS Docker applications";
    wantedBy = [ "multi-user.target" ];
    after = [ "docker.service" ];
    requires = [ "docker.service" ];
    unitConfig = {
      ConditionPathExists = applicationSettings;
      RequiresMountsFor = applicationRoot;
    };
    serviceConfig = {
      User = "root";
      UMask = "0077";
      RuntimeDirectory = "anima-applications";
      RuntimeDirectoryMode = "0755";
      Restart = "on-abnormal";
      ExecStart = serveApplications;
    };
  };
}
