{ ... }:
let
  hostingMemoryLow = "50%";
in
{
  systemd.slices = {
    # Memory protection is capped by each ancestor's effective protection.
    anima.sliceConfig.MemoryLow = hostingMemoryLow;
    anima-hosting = {
      description = "Protected hosted applications";
      sliceConfig = { CPUWeight = 1000; IOWeight = 1000; MemoryLow = hostingMemoryLow; };
    };
    anima-agents = {
      description = "Preemptible managed agent and build work";
      sliceConfig = {
        CPUWeight = 10;
        IOWeight = 10;
        MemoryHigh = "25%";
        MemoryMax = "35%";
        TasksMax = 1024;
      };
    };
  };
  virtualisation.docker.daemon.settings.cgroup-parent = "anima-hosting.slice";
  systemd.services.docker = {
    requires = [ "anima-hosting.slice" ];
    after = [ "anima-hosting.slice" ];
  };
  # These are aggregate ceilings, not permission to launch work. Admission must
  # still reserve measured available capacity before creating an agent scope.
  environment.etc."anima/admission.json".text = builtins.toJSON {
    schema = 1;
    protectedMemoryPercent = 65;
    maximumAgentMemoryPercent = 25;
    maximumAgentCpuPercent = 25;
    maximumAgentTasks = 512;
    agentSlice = "anima-agents.slice";
    hostingSlice = "anima-hosting.slice";
  };
}
