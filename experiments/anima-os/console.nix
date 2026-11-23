{ config, pkgs, lib, workspaceRoot ? ../.., ... }:
let
  session = import ./session-package.nix { inherit pkgs workspaceRoot; };
  python = import ./console-python.nix { inherit pkgs; };
  source = pkgs.runCommand "anima-local-console-source" {} ''
    mkdir -p $out
    cp ${./console}/app.py ${./console}/signin.py ${./console}/screen.py ${./console}/*.js ${./console}/*.html ${./console}/*.css $out/
    rm -f $out/test_*.py
    cp ${./branding/hero.png} $out/hero.png
    cp ${./branding/wordmark.svg} $out/wordmark.svg
    cp ${./telemetry/readings.py} $out/readings.py
  '';
  privileged = pkgs.writeShellApplication {
    name = "anima-console-privileged";
    runtimeInputs = with pkgs; [ networkmanager systemd util-linux xrandr xinput coreutils ];
    text = ''
      ${lib.optionalString config.services.anima-os-control.enable "export ANIMA_OS_CONTROL=${config.services.anima-os-control.package}/bin/anima-osd"}
      exec ${session}/bin/anima-os-session
    '';
  };
  console = pkgs.writeShellApplication {
    name = "anima-console";
    runtimeInputs = [ python pkgs.xrandr pkgs.xset ];
    text = ''
      ulimit -c 0
      export QT_FONT_DPI=96
      QT_SCALE_FACTOR=$(python3 ${./display/scale.py})
      export QT_SCALE_FACTOR
      export QT_PLUGIN_PATH="${pkgs.qt6.qtbase}/${pkgs.qt6.qtbase.qtPluginPrefix}"
      export QTWEBENGINE_RESOURCES_PATH="${pkgs.qt6.qtwebengine}/resources"
      export QTWEBENGINE_LOCALES_PATH="${pkgs.qt6.qtwebengine}/qtwebengine_locales"
      export QTWEBENGINEPROCESS_PATH="${pkgs.qt6.qtwebengine}/libexec/QtWebEngineProcess"
      exec python3 ${source}/app.py
    '';
  };
in {
  services.xserver.enable = true;
  services.xserver.windowManager.openbox.enable = true;
  services.xserver.displayManager.lightdm.enable = true;
  services.displayManager.defaultSession = "none+openbox";
  services.displayManager.autoLogin = { enable = true; user = "admin"; };
  environment.systemPackages = [ console privileged pkgs.xdotool ];
  fonts.packages = [ pkgs.ibm-plex ];
  security.sudo.extraConfig = ''
    Defaults:admin timestamp_timeout=0,passwd_tries=1
  '';
  services.xserver.displayManager.sessionCommands = ''
    ${pkgs.xset}/bin/xset s off -dpms
    ${pkgs.xsetroot}/bin/xsetroot -solid '#1B1D21'
    (while true; do ${console}/bin/anima-console; sleep 1; done) &
  '';
  environment.etc."xdg/openbox/rc.xml".text = ''
    <openbox_config xmlns="http://openbox.org/3.4/rc"><applications><application class="*"><decor>no</decor></application></applications><keyboard/><mouse/><menu/></openbox_config>
  '';
}
