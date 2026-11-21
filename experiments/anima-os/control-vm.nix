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
    machine.succeed("mkdir -p /persist")
    adapter = machine.succeed("PYTHONPATH=${./console}:${./display} ANIMA_OS_CONTROL=${package}/bin/anima-osd ${pkgs.python3}/bin/python3 -c 'import json; from privileged import Session; print(json.dumps(Session().dispatch(\"diagnostics\", {})))'")
    adapter = json.loads(adapter)
    assert adapter["monitoring"] == "active"
    assert adapter["private_link"] == "failed"
    assert "network" in adapter and "root" in adapter
    machine.succeed("systemctl stop anima-os-control.service")
    machine.fail("test -e /run/anima-osd/control.sock")
  '';
}
