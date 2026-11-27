# Disposable real-kernel fixture for the Rust controller's boot transaction.
{ pkgs, workspaceRoot }:
let
  keys = pkgs.runCommand "anima-boot-synthetic-key" { nativeBuildInputs = [ pkgs.openssl ]; } ''
    mkdir -p $out
    openssl genpkey -algorithm RSA -pkeyopt rsa_keygen_bits:2048 -out $out/private.pem 2>/dev/null
    openssl pkey -in $out/private.pem -pubout -out $out/public.pem
  '';
  package = import ../../packages/update-package.nix { inherit pkgs workspaceRoot; publicKey = "${keys}/public.pem"; };
  system = revision: (import (pkgs.path + "/nixos/lib/eval-config.nix") {
    system = pkgs.stdenv.hostPlatform.system;
    modules = [ ./machine.nix ../../modules/updates.nix ({ lib, ... }: {
      anima.revision = revision;
      networking.hostName = lib.mkForce "anima";
      services.anima-os-updates.package = package;
      environment.etc."anima-preview".text = "disposable-update-boot-fixture";
      environment.etc."anima/docker-client/config.json".text = "{}";
      environment.systemPackages = [ pkgs.python3 pkgs.openssl pkgs.xz pkgs.nix ];
      virtualisation.docker.enable = true;
      systemd.tmpfiles.rules = [
        "d /run/anima-os 0700 root root -"
        "d /persist/anima-admission/reservations 0700 root root -"
      ];
      # The real root, kernel, EFI and update service run unchanged. These
      # headless services supply the graphical appliance's health dependencies.
      systemd.services = builtins.listToAttrs (map (name: {
        inherit name;
        value = { wantedBy = [ "multi-user.target" ]; serviceConfig = {
          Type = "oneshot"; RemainAfterExit = true; ExecStart = "${pkgs.coreutils}/bin/true";
        }; };
      }) [ "NetworkManager" "wpa_supplicant" "display-manager" ]) // {
        anima-local-health.enable = lib.mkForce false;
        anima-ready = {
          wantedBy = [ "multi-user.target" ];
          after = [ "anima-root-lock.service" "anima-update-health.service" "docker.service" ];
          requires = [ "anima-root-lock.service" "anima-update-health.service" "docker.service" ];
          serviceConfig.Type = "oneshot";
          script = "echo ANIMA_UPDATE_READY_${revision} >/dev/ttyS0";
        };
      };
    }) ];
  }).config;
  a = system "A";
  b = system "B";
  image = import ./image.nix {
    inherit pkgs;
    systemA = a.system.build.toplevel; systemB = b.system.build.toplevel;
    paramsA = a.boot.kernelParams; paramsB = b.boot.kernelParams;
  };
in pkgs.runCommand "anima-rust-update-boot-fixture" {} ''
  mkdir -p $out
  ln -s ${image}/anima.qcow2 $out/anima.qcow2
  ln -s ${keys} $out/keys
  echo ${b.system.build.toplevel} > $out/system-b
  echo ${a.system.build.toplevel} > $out/system-a
''
