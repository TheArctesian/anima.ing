{ pkgs, workspaceRoot }:
let
  package = import ../../packages/session-package.nix { inherit pkgs workspaceRoot; };
  execution = import ../../packages/execution-package.nix { inherit pkgs workspaceRoot; };
  docker = pkgs.writeShellScriptBin "docker" ''
    test "$*" = "--config /etc/anima/docker-client --host unix:///run/docker.sock ps --quiet"
    if [ -f /run/anima-maintenance-fixture-docker ]; then
      ${pkgs.coreutils}/bin/cat /run/anima-maintenance-fixture-docker
    fi
  '';
  update = pkgs.writeShellScriptBin "anima-os-update" ''
    if [ "$1" = download ]; then ${pkgs.coreutils}/bin/sleep 2; fi
    echo '{"ok":true,"value":{"phase":"idle"}}'
  '';
  scenario = pkgs.writeText "session-check.py" ''
    import json, os, subprocess, time
    def start():
        child = subprocess.Popen(['${package}/bin/anima-os-session'], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True, env=dict(os.environ, DISPLAY=':99'))
        assert json.loads(child.stdout.readline())['id'] == 'unlock'
        return child
    def send(child, identity, op, args={}):
        child.stdin.write(json.dumps(dict(id=identity,op=op,args=args))+'\n'); child.stdin.flush()
    def read(child):
        return json.loads(child.stdout.readline())
    child = start()
    send(child,'bad','connect',dict(device='missing',bssid='bad',password='bad\nsecret'))
    assert not read(child)['ok']
    send(child,'origin','enroll-selfhost',dict(hub='https://user:password@invalid.example',code='anima_enrol_synthetic'))
    assert not read(child)['ok']
    send(child,'power','restart')
    assert not read(child)['ok']
    send(child,'download','update-download')
    send(child,'observe','update-status')
    before=time.monotonic(); reply=read(child)
    assert reply['id']=='observe' and reply['ok'] and time.monotonic()-before<1
    send(child,'competing','update-check')
    denied=read(child); assert denied['id']=='competing' and not denied['ok']
    assert read(child)['id']=='download'
    send(child,'displays','displays')
    display=read(child)['value'][0]['name']
    send(child,'preview','rotate',dict(output=display,rotation='normal'))
    assert read(child)['value']['revertSeconds']==15
    send(child,'keep','keep-rotation')
    assert read(child)['ok']
    assert json.load(open('/persist/display.json'))[display]=='normal'
    send(child,'preview2','rotate',dict(output=display,rotation='normal'))
    assert read(child)['ok']
    child.kill(); child.wait()
    deadline=time.monotonic()+25
    while os.path.exists('/run/anima-display-preview.json') and time.monotonic()<deadline:
        time.sleep(.1)
    assert not os.path.exists('/run/anima-display-preview.json'), 'independent display guard did not restore'
    child=start(); send(child,'expired','keep-rotation'); assert not read(child)['ok']
    child.stdin.close(); assert child.wait(timeout=5)==0
    env=dict(os.environ, DISPLAY=':99')
    subprocess.run(['${package}/bin/anima-os-session','display-set',display,'normal'],env=env,check=True)
    subprocess.run(['${package}/bin/anima-os-session','display-keep',display],env=env,check=True)
    assert not os.path.exists('/run/anima-display-preview.json')
  '';
in pkgs.testers.runNixOSTest {
  name = "anima-os-session";
  nodes.machine = {
    virtualisation.fileSystems."/persist" = { device = "tmpfs"; fsType = "tmpfs"; options = [ "mode=0755" ]; };
    networking.networkmanager.enable = true;
    users.users.observer.isNormalUser = true;
    environment.systemPackages = with pkgs; [ package update docker networkmanager systemd util-linux xrandr xinput coreutils python3 ];
    systemd.services.fixture-display = {
      wantedBy = [ "multi-user.target" ];
      serviceConfig.ExecStart = "${pkgs.xorg.xorgserver}/bin/Xvfb :99 -screen 0 1280x800x24 -ac";
    };
    systemd.tmpfiles.rules = [
      "d /persist 0755 root root -"
      "d /persist/anima-admission/reservations 0700 root root -"
      "d /persist/anima-console-activity 0700 root root -"
      "d /persist/anima-workspaces/locks 0700 root root -"
      "d /run/anima-os 0700 root root -"
    ];
    virtualisation.memorySize = 1024;
  };
  testScript = ''
    machine.start()
    machine.wait_for_unit("fixture-display.service")
    machine.wait_for_unit("NetworkManager.service")
    machine.succeed("findmnt --mountpoint /persist")
    machine.fail("su -s /bin/sh observer -c '${package}/bin/anima-os-session'")
    machine.wait_until_succeeds("DISPLAY=:99 xrandr --prop")
    machine.succeed("${pkgs.python3}/bin/python3 ${scenario}", timeout=80)
    machine.succeed("ANIMA_ACTIVITY_FIXTURE=1 ${pkgs.python3}/bin/python3 ${../session/activity-check.py} ${package}/bin/anima-os-session")
    machine.succeed("ANIMA_MAINTENANCE_FIXTURE=1 ${pkgs.python3}/bin/python3 ${../session/maintenance-check.py} ${package}/bin/anima-os-session ${execution}/bin/anima-os-execution")
  '';
}
