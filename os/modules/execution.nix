{ pkgs, lib, daemonPackage, workspaceRoot, ... }:
let
  image = import ../images/workspace-image.nix { inherit pkgs daemonPackage; };
  firewall = pkgs.writeText "anima-workspace-network.nft" ''
    destroy table inet anima_workspaces
    table inet anima_workspaces {
      chain isolate_host {
        type filter hook input priority -5; policy accept;
        iifname "anaw*" drop
      }
      chain public_egress {
        type filter hook forward priority -5; policy accept;
        iifname "anaw*" ip daddr { 0.0.0.0/8, 10.0.0.0/8, 100.64.0.0/10, 127.0.0.0/8, 169.254.0.0/16, 172.16.0.0/12, 192.168.0.0/16, 224.0.0.0/4, 240.0.0.0/4 } drop
        iifname "anaw*" ip6 daddr { ::/128, ::1/128, fc00::/7, fe80::/10, ff00::/8 } drop
      }
    }
  '';
  binary = import ../packages/execution-package.nix { inherit pkgs workspaceRoot; };

in {
  imports = [ (import ./source-jobs.nix { inherit binary; }) ];
  nixpkgs.config.allowUnfreePredicate = package: lib.getName package == "claude-code";
  environment.systemPackages = [ binary pkgs.btrfs-progs ];
  environment.etc."anima/workspace-image".text = "${image.imageName}:${image.imageTag}\n";
  systemd.tmpfiles.rules = [
    "d /persist/anima-source-jobs 0700 root root -"
    "d /persist/anima-work-policy 0700 root root -"
    "d /persist/anima-commands 0700 root root -"
    "d /persist/anima-workspace-requests 0700 root root -"
    "d /run/anima-previews 0700 root root -"
    "d /persist/anima-workspaces 0700 root root -"
    "d /persist/anima-workspaces/homes 0700 root root -"
    "d /persist/anima-workspaces/locks 0700 root root -"
  ];
  systemd.services.anima-execution-broker = {
    description = "Managed Code boundary for the enrolled node daemon";
    wantedBy = [ "multi-user.target" ];
    requires = [ "anima-execution-recovery.service" ];
    after = [ "anima-execution-recovery.service" ];
    serviceConfig = {
      ExecStart = "${binary}/bin/anima-os-execution broker";
      User = "root";
      Group = "anima-daemon";
      RuntimeDirectory = "anima-execution";
      RuntimeDirectoryMode = "0750";
      Restart = "on-failure";
      RestartSec = 5;
      UMask = "0077";
      NoNewPrivileges = true;
      CapabilityBoundingSet = "";
      ProtectSystem = "strict";
      ProtectHome = true;
      ReadWritePaths = [ "/persist/anima-commands" "/persist/anima-workspaces/locks" "/run/anima-previews" ];
      PrivateTmp = true;
      ProtectKernelTunables = true;
      ProtectKernelModules = true;
      ProtectControlGroups = true;
      RestrictAddressFamilies = [ "AF_UNIX" ];
      RestrictSUIDSGID = true;
      LockPersonality = true;
      MemoryMax = "256M";
      TasksMax = 128;
      Nice = 10;
    };
  };
  systemd.services.anima-workspace-broker = {
    description = "Authorized managed workspace lifecycle";
    wantedBy = [ "multi-user.target" ];
    requires = [ "anima-execution-recovery.service" "anima-workspace-image.service" ];
    after = [ "anima-execution-recovery.service" "anima-workspace-image.service" ];
    serviceConfig = {
      ExecStart = "${binary}/bin/anima-os-execution workspace-broker";
      User = "root";
      Group = "anima-daemon";
      RuntimeDirectory = "anima-workspace";
      RuntimeDirectoryMode = "0750";
      Restart = "on-failure";
      RestartSec = 5;
      UMask = "0077";
      NoNewPrivileges = true;
      # Btrfs subvolume quota ioctls and initial workspace UID ownership only.
      CapabilityBoundingSet = [ "CAP_SYS_ADMIN" "CAP_CHOWN" "CAP_DAC_OVERRIDE" ];
      ProtectSystem = "strict";
      ProtectHome = true;
      ReadWritePaths = [ "/persist/anima-workspaces" "/persist/anima-workspace-requests" "/persist/anima-admission" "/run/anima-os" ];
      PrivateTmp = true;
      ProtectKernelTunables = true;
      ProtectKernelModules = true;
      ProtectControlGroups = true;
      RestrictAddressFamilies = [ "AF_UNIX" ];
      RestrictSUIDSGID = true;
      LockPersonality = true;
      MemoryMax = "256M";
      TasksMax = 128;
      Nice = 10;
      IOSchedulingClass = "idle";
    };
  };
  systemd.services.anima-daemon.environment.ANIMA_MANAGED_CODE = "1";
  systemd.services.anima-link.environment.ANIMA_MANAGED_PREVIEWS = "1";
  systemd.services.anima-link.environment.ANIMA_MANAGED_PROVIDER_LOGIN = "1";
  systemd.services.anima-link.wants = [ "anima-execution-broker.service" ];
  systemd.services.anima-link.after = [ "anima-execution-broker.service" ];
  systemd.services.anima-daemon.wants = [ "anima-execution-broker.service" "anima-workspace-broker.service" ];
  systemd.services.anima-daemon.after = [ "anima-execution-broker.service" "anima-workspace-broker.service" ];
  systemd.services.anima-build-leases = {
    description = "Enforce local deadlines for leased source builds";
    wantedBy = [ "multi-user.target" ];
    requires = [ "anima-execution-recovery.service" ];
    after = [ "anima-execution-recovery.service" ];
    unitConfig.RequiresMountsFor = "/persist";
    serviceConfig = {
      ExecStart = "${binary}/bin/anima-os-execution build-watch";
      Restart = "always";
      RestartSec = 5;
      User = "root";
      UMask = "0077";
      NoNewPrivileges = true;
      CapabilityBoundingSet = "";
      ProtectSystem = "strict";
      ProtectHome = true;
      ReadWritePaths = [ "/persist/anima-source-jobs" "/persist/anima-builds" "/persist/anima-admission" "/persist/anima-workspaces" "/run/anima-os" ];
      PrivateTmp = true;
      ProtectKernelTunables = true;
      ProtectKernelModules = true;
      ProtectControlGroups = true;
      RestrictAddressFamilies = [ "AF_UNIX" ];
      MemoryMax = "128M";
      TasksMax = 32;
      Nice = 10;
    };
  };
  systemd.services.anima-hosting-pressure = {
    description = "Throttle managed work during hosting memory pressure";
    wantedBy = [ "multi-user.target" ];
    bindsTo = [ "anima-hosting.slice" "anima-agents.slice" ];
    after = [ "anima-hosting.slice" "anima-agents.slice" ];
    serviceConfig = {
      ExecStart = "${binary}/bin/anima-os-execution hosting-pressure";
      User = "root";
      Restart = "on-failure";
      RestartSec = 5;
      UMask = "0077";
      NoNewPrivileges = true;
      CapabilityBoundingSet = "";
      ProtectSystem = "strict";
      ProtectHome = true;
      ProtectKernelTunables = true;
      ProtectKernelModules = true;
      ReadOnlyPaths = [ "/sys/fs/cgroup" ];
      ReadWritePaths = [ "/sys/fs/cgroup/anima.slice/anima-agents.slice/memory.high" ];
      RuntimeDirectory = "anima-hosting-pressure";
      RuntimeDirectoryMode = "0700";
      RuntimeDirectoryPreserve = "yes";
      PrivateTmp = true;
      RestrictAddressFamilies = [ "AF_UNIX" ];
      MemoryMax = "64M";
      TasksMax = 16;
    };
  };
  systemd.services.anima-execution-recovery = {
    description = "Reconcile managed work before opening node admission";
    wantedBy = [ "multi-user.target" ];
    requires = [ "docker.service" "anima-workspace-storage.service" ];
    after = [ "docker.service" "anima-workspace-storage.service" "systemd-tmpfiles-setup.service" ];
    unitConfig.RequiresMountsFor = "/persist";
    serviceConfig = {
      Type = "oneshot";
      RemainAfterExit = true;
      ExecStart = "${binary}/bin/anima-os-execution recover";
      TimeoutStartSec = "10min";
      Nice = 15;
      IOSchedulingClass = "idle";
      UMask = "0077";
    };
  };
  systemd.services.anima-workspace-storage = {
    description = "Enable bounded storage for isolated workspaces";
    wantedBy = [ "multi-user.target" ];
    before = [ "docker.service" ];
    after = [ "local-fs.target" ];
    unitConfig.RequiresMountsFor = "/persist";
    serviceConfig = { Type = "oneshot"; RemainAfterExit = true; };
    script = "${pkgs.btrfs-progs}/bin/btrfs quota enable /persist";
  };
  systemd.services.anima-workspace-image = {
    description = "Load the offline Anima workspace image";
    wantedBy = [ "multi-user.target" ];
    requires = [ "docker.service" "anima-workspace-storage.service" ];
    after = [ "docker.service" "anima-workspace-storage.service" ];
    serviceConfig = { Type = "oneshot"; RemainAfterExit = true; TimeoutStartSec = "30min"; Nice = 10; IOSchedulingClass = "idle"; };
    script = ''
      docker=${pkgs.docker}/bin/docker
      reference='${image.imageName}:${image.imageTag}'
      if ! "$docker" image inspect "$reference" >/dev/null 2>&1; then
        "$docker" load --input ${image}
      fi
      # Docker can register a tag even when its layer unpack fails.
      exec "$docker" run --rm --pull=never --network=none --read-only \
        --cgroup-parent=anima-agents.slice --memory=64m --pids-limit=16 \
        --cap-drop=ALL --security-opt=no-new-privileges \
        --entrypoint /bin/true "$reference"
    '';
  };
  systemd.services.anima-workspace-network = {
    description = "Restrict workspace networks to public egress";
    wantedBy = [ "multi-user.target" ];
    before = [ "docker.service" ];
    after = [ "network-pre.target" ];
    serviceConfig = { Type = "oneshot"; RemainAfterExit = true; };
    script = "${pkgs.nftables}/bin/nft -f ${firewall}";
  };
  systemd.services.docker.requires = [ "anima-workspace-network.service" ];
}
