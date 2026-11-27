{ pkgs, workspaceRoot }:
let
  package = import ../../packages/control-package.nix { inherit pkgs workspaceRoot; };
  session = import ../../packages/session-package.nix { inherit pkgs workspaceRoot; };
  adapter = ../session/diagnostic-check.py;
in pkgs.testers.runNixOSTest {
  name = "anima-os-control";
  nodes.machine = {
    imports = [ ../../modules/control.nix ];
    services.anima-os-control = { enable = true; publishConsoleSnapshot = true; inherit package; };
    networking.networkmanager.enable = true;
    environment.systemPackages = [ pkgs.python3 pkgs.util-linux pkgs.networkmanager ];
    users.users.observer = { isNormalUser = true; };
    users.users.admin = { isNormalUser = true; };
    systemd.services.anima-daemon = {
      wantedBy = [ "multi-user.target" ];
      serviceConfig.ExecStart = "${pkgs.coreutils}/bin/sleep infinity";
    };
    systemd.services.anima-link.serviceConfig = { Type = "oneshot"; ExecStart = "${pkgs.coreutils}/bin/false"; };
    virtualisation.memorySize = 1024;
    virtualisation.emptyDiskImages = [ 64 ];
    virtualisation.fileSystems."/persist" = {
      device = "/dev/vdb";
      fsType = "ext4";
      autoFormat = true;
      neededForBoot = true;
    };
  };
  testScript = ''
    import json
    machine.start()
    machine.wait_for_unit("anima-os-control.service")
    status = json.loads(machine.succeed("anima-osd status"))
    assert status["result"] == "status"
    assert status["value"]["memory_total_bytes"] > 0
    machine.fail("su -s /bin/sh observer -c 'anima-osd status'")
    snapshot = "/run/anima-observation/console.json"
    machine.wait_until_succeeds(f"su -s /bin/sh admin -c 'test -r {snapshot}'")
    local = json.loads(machine.succeed(f"su -s /bin/sh admin -c 'cat {snapshot}'"))
    assert local["schema"] == 1 and local["memoryTotal"] > 0
    machine.fail("su -s /bin/sh admin -c 'anima-osd status'")
    machine.fail(f"su -s /bin/sh admin -c 'test -w {snapshot}'")
    machine.fail(f"su -s /bin/sh observer -c 'test -r {snapshot}'")
    machine.succeed("systemctl restart anima-os-control.service")
    machine.wait_for_unit("anima-os-control.service")
    machine.wait_until_succeeds("anima-osd status")
    machine.wait_for_unit("anima-daemon.service")
    machine.fail("systemctl start anima-link.service")
    diagnostics = json.loads(machine.succeed("anima-osd diagnostics"))
    assert diagnostics["result"] == "diagnostics"
    assert diagnostics["value"]["monitoring"] == "active"
    assert diagnostics["value"]["private_link"] == "failed"
    request = "6fa05258-890a-48d9-b7da-9719b9a0367e"
    machine.fail(f"su -s /bin/sh observer -c 'anima-osd job-start {request}'")
    started = json.loads(machine.succeed(f"anima-osd job-start {request}"))
    assert started["version"] == 2 and started["result"] == "operation"
    operation_id = started["value"]["id"]
    machine.wait_until_succeeds(f"anima-osd job-get {request} | python3 -c 'import json,sys; assert json.load(sys.stdin)[\"value\"][\"state\"] == \"succeeded\"'")
    completed = json.loads(machine.succeed(f"anima-osd job-get {request}"))["value"]
    assert completed["result"]["monitoring"] == "active"
    assert completed["result"]["private_link"] == "failed"
    repeated = json.loads(machine.succeed(f"anima-osd job-start {request}"))["value"]
    assert repeated["id"] == operation_id
    machine.succeed("systemctl restart anima-os-control.service")
    machine.wait_until_succeeds("anima-osd status")
    retained = json.loads(machine.succeed(f"anima-osd job-get {request}"))["value"]
    assert retained == completed
    machine.reboot()
    machine.wait_for_shutdown()
    machine.start()
    machine.wait_for_unit("anima-os-control.service")
    machine.wait_until_succeeds("anima-osd status")
    retained = json.loads(machine.succeed(f"anima-osd job-get {request}"))["value"]
    assert retained == completed
    machine.fail("su -s /bin/sh observer -c 'ls /persist/anima-osd-jobs'")
    machine.wait_for_unit("anima-daemon.service")
    machine.fail("systemctl start anima-link.service")
    machine.succeed("${pkgs.python3}/bin/python3 ${adapter} ${session}/bin/anima-os-session ${package}/bin/anima-osd")
    machine.succeed("systemctl stop anima-os-control.service")
    machine.fail("test -e /run/anima-osd/control.sock")
  '';
}
