{ binary }:
{
  systemd.services.anima-source-jobs = {
    description = "Enrolled daemon source-job lifecycle boundary";
    wantedBy = [ "multi-user.target" ];
    requires = [ "anima-execution-recovery.service" "anima-build-leases.service" ];
    after = [ "anima-execution-recovery.service" "anima-build-leases.service" ];
    serviceConfig = {
      ExecStart = "${binary}/bin/anima-os-execution source-job-broker";
      User = "root";
      Group = "anima-daemon";
      RuntimeDirectory = "anima-source-jobs";
      RuntimeDirectoryMode = "0750";
      Restart = "on-failure";
      RestartSec = 5;
      UMask = "0077";
      NoNewPrivileges = true;
      CapabilityBoundingSet = "";
      ProtectSystem = "strict";
      ProtectHome = true;
      ReadWritePaths = [
        "/persist/anima-source-jobs" "/persist/anima-work-policy"
        "/persist/anima-workspaces" "/persist/anima-builds"
        "/persist/anima-admission" "/run/anima-os"
      ];
      PrivateTmp = true;
      ProtectKernelTunables = true;
      ProtectKernelModules = true;
      ProtectControlGroups = true;
      RestrictAddressFamilies = [ "AF_UNIX" ];
      RestrictSUIDSGID = true;
      LockPersonality = true;
      MemoryMax = "256M";
      TasksMax = 64;
      CPUQuota = "25%";
      Nice = 10;
    };
  };
  systemd.services.anima-source-cleanup = {
    description = "Reconcile durable source-job storage releases";
    wantedBy = [ "multi-user.target" ];
    requires = [ "anima-execution-recovery.service" "docker.service" ];
    after = [ "anima-execution-recovery.service" "docker.service" ];
    serviceConfig = {
      ExecStart = "${binary}/bin/anima-os-execution source-job-cleanup";
      User = "root";
      Slice = "anima-agents.slice";
      Restart = "on-failure";
      RestartSec = 5;
      UMask = "0077";
      NoNewPrivileges = true;
      CapabilityBoundingSet = [ "CAP_SYS_ADMIN" "CAP_DAC_OVERRIDE" ];
      ProtectSystem = "strict";
      ProtectHome = true;
      ReadWritePaths = [
        "/persist/anima-source-jobs" "/persist/anima-workspaces" "/persist/anima-builds"
        "/persist/anima-admission" "/run/anima-os"
      ];
      PrivateTmp = true;
      ProtectKernelTunables = true;
      ProtectKernelModules = true;
      ProtectControlGroups = true;
      RestrictAddressFamilies = [ "AF_UNIX" ];
      RestrictSUIDSGID = true;
      LockPersonality = true;
      MemoryMax = "128M";
      TasksMax = 32;
      CPUQuota = "25%";
      Nice = 19;
      IOSchedulingClass = "idle";
    };
  };
  systemd.services.anima-daemon.wants = [ "anima-source-jobs.service" ];
}
