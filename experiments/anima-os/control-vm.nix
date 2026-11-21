{ pkgs, workspaceRoot }:
let
  package = import ./control-package.nix { inherit pkgs workspaceRoot; };
in pkgs.testers.runNixOSTest {
  name = "anima-os-control";
  nodes.machine = {
    imports = [ ./control.nix ];
    services.anima-os-control = { enable = true; inherit package; };
    networking.networkmanager.enable = true;
    environment.systemPackages = [ pkgs.python3 pkgs.util-linux pkgs.networkmanager ];
    users.users.observer = { isNormalUser = true; };
    systemd.services.anima-daemon = {
      wantedBy = [ "multi-user.target" ];
      serviceConfig.ExecStart = "${pkgs.coreutils}/bin/sleep infinity";
    };
    systemd.services.anima-link.serviceConfig = { Type = "oneshot"; ExecStart = "${pkgs.coreutils}/bin/false"; };
    virtualisation.memorySize = 1024;
    virtualisation.emptyDiskImages = [ 64 ];
    fileSystems."/persist" = {
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
    machine.wait_for_unit("anima-os-control.service")
    machine.wait_until_succeeds("anima-osd status")
    retained = json.loads(machine.succeed(f"anima-osd job-get {request}"))["value"]
    assert retained == completed
    machine.fail("su -s /bin/sh observer -c 'ls /persist/anima-osd-jobs'")
    machine.wait_for_unit("anima-daemon.service")
    machine.fail("systemctl start anima-link.service")
    adapter = machine.succeed("PYTHONPATH=${./console}:${./display} ANIMA_OS_CONTROL=${package}/bin/anima-osd ${pkgs.python3}/bin/python3 -c 'import json; from privileged import Session; print(json.dumps(Session().dispatch(\"diagnostics\", {})))'")
    adapter = json.loads(adapter)
    assert adapter["monitoring"] == "active"
    assert adapter["private_link"] == "failed"
    assert "network" in adapter and "root" in adapter
    machine.succeed("systemctl stop anima-os-control.service")
    machine.fail("test -e /run/anima-osd/control.sock")
  '';
}
