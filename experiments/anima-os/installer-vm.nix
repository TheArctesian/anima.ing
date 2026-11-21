{ pkgs, workspaceRoot }:
let
  package = import ./installer-package.nix { inherit pkgs workspaceRoot; };
  installed = (import "${pkgs.path}/nixos/lib/eval-config.nix" {
    system = pkgs.stdenv.hostPlatform.system;
    modules = [{
      boot.loader.systemd-boot.enable = true;
      boot.loader.efi.canTouchEfiVariables = false;
      boot.initrd.availableKernelModules = [ "virtio_pci" "virtio_blk" ];
      fileSystems = {
        "/" = { device = "/dev/disk/by-label/ANIMA_OS"; fsType = "btrfs"; options = [ "subvol=@root" ]; };
        "/nix" = { device = "/dev/disk/by-label/ANIMA_OS"; fsType = "btrfs"; options = [ "subvol=@nix" ]; };
        "/var" = { device = "/dev/disk/by-label/ANIMA_OS"; fsType = "btrfs"; options = [ "subvol=@var" ]; };
        "/persist" = { device = "/dev/disk/by-label/ANIMA_OS"; fsType = "btrfs"; options = [ "subvol=@persist" ]; };
        "/boot" = { device = "/dev/disk/by-label/ANIMA_EFI"; fsType = "vfat"; };
      };
      environment.etc."anima-preview".text = "disposable-installer-test";
      system.stateVersion = "26.05";
    }];
  }).config.system.build.toplevel;
  scenario = pkgs.writeText "installer-check.py" ''
    import fcntl, hashlib, json, pathlib, subprocess
    def command(*args):
        return subprocess.check_output(args, text=True).strip()
    def inventory():
        return json.loads(command('${package}/bin/anima-os-install', 'inventory'))
    def identity(disk):
        return {key: disk.get(key) for key in ('path','maj:min','size','model','serial')}
    def fingerprint(device="/dev/vdb", size=64):
        with open(device, 'rb', buffering=0) as disk:
            first = disk.read(1024*1024)
            disk.seek(size*1024**3-1024*1024)
            return hashlib.sha256(first+disk.read(1024*1024)).hexdigest()
    def execute(request):
        child = subprocess.run(['${package}/bin/anima-os-install','install','--system','${installed}'], input=json.dumps(request), text=True, capture_output=True)
        assert 'synthetic-owner' not in child.stdout + child.stderr
        return child
    devices = {disk['path']: disk for disk in inventory()}
    assert not devices['/dev/vda']['eligible']
    assert devices['/dev/vdb']['eligible'] and devices['/dev/vdc']['eligible']
    selected = devices['/dev/vdb']
    password = command('mkpasswd','--method=yescrypt','synthetic-owner-password')
    request = dict(system='${installed}',selected=selected,data=[],confirmed=[],hashed_password=password,keymap='keycode 1 = Escape\n')
    before = fingerprint()
    sentinel = fingerprint('/dev/vdd', 32)
    assert execute(request).returncode != 0
    assert fingerprint() == before
    request['confirmed'] = [identity(selected)]
    with open('/run/anima-install-operation.lock', 'w') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        blocked = execute(request)
        assert blocked.returncode != 0 and 'already running' in blocked.stdout
    assert fingerprint() == before
    request['data'] = [dict(devices['/dev/vdc'],serial='changed')]
    request['confirmed'].append(identity(request['data'][0]))
    assert execute(request).returncode != 0
    assert fingerprint() == before
    request['data'] = [devices['/dev/vdc']]
    request['confirmed'][1] = identity(request['data'][0])
    pathlib.Path('/etc/NetworkManager/system-connections').mkdir(parents=True,exist_ok=True)
    pathlib.Path('/etc/NetworkManager/system-connections/synthetic.nmconnection').write_text('[connection]\nid=synthetic\n')
    pathlib.Path('/etc/NetworkManager/system-connections/unsafe.nmconnection').symlink_to('/etc/passwd')
    result = execute(request)
    print(result.stdout)
    assert result.returncode == 0, result.stdout
    assert fingerprint('/dev/vdd', 32) == sentinel
    assert json.loads(result.stdout.splitlines()[-1])['state'] == 'succeeded'
    assert command('blkid','-s','LABEL','-o','value','/dev/vdb1') == 'ANIMA_EFI'
    assert command('blkid','-s','LABEL','-o','value','/dev/vdb2') == 'ANIMA_OS'
    pathlib.Path('/persist').mkdir(exist_ok=True)
    command('mount','-t','btrfs','-o','subvol=@persist','/dev/vdb2','/persist')
    owner = pathlib.Path('/persist/owner-password')
    assert owner.stat().st_mode & 0o777 == 0o600
    assert owner.read_text().strip() == password
    assert pathlib.Path('/persist/network-connections/synthetic.nmconnection').stat().st_mode & 0o777 == 0o600
    assert not pathlib.Path('/persist/network-connections/unsafe.nmconnection').exists()
    records = json.loads(pathlib.Path('/persist/data-volumes.json').read_text())
    assert len(records) == 1
    command('${package}/bin/anima-os-install','mount-volumes')
    command('${package}/bin/anima-os-install','mount-volumes')
    assert command('findmnt','-n','-o','UUID','--mountpoint','/persist/volumes/'+records[0]['uuid']) == records[0]['uuid']
    command('umount','/persist/volumes/'+records[0]['uuid'])
    command('mkfs.btrfs','-f','-U',records[0]['uuid'],'/dev/vdd')
    command('udevadm','settle')
    assert subprocess.run(['${package}/bin/anima-os-install','mount-volumes']).returncode != 0
    pathlib.Path('/persist/data-volumes.json').write_text('[{"uuid":"../escape"}]')
    assert subprocess.run(['${package}/bin/anima-os-install','mount-volumes']).returncode != 0
    command('umount','/persist')
  '';
in pkgs.testers.runNixOSTest {
  name = "anima-os-native-installer";
  nodes.machine = {
    environment.systemPackages = with pkgs; [ package python3 mkpasswd util-linux parted btrfs-progs dosfstools ];
    virtualisation = {
      memorySize = 2048;
      diskSize = 16000;
      emptyDiskImages = [ 65536 32768 32768 ];
      useEFIBoot = true;
    };
    system.extraDependencies = [ installed ];
  };
  testScript = ''
    machine.start()
    machine.wait_for_unit("multi-user.target")
    machine.succeed("test -d /sys/firmware/efi")
    machine.succeed("${pkgs.python3}/bin/python3 ${scenario}", timeout=1200)
  '';
}
