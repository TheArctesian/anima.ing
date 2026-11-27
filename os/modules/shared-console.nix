{ pkgs, ... }:
let
  python = import ../packages/console-python.nix { inherit pkgs; };
  physicalSource = pkgs.runCommand "anima-physical-console-source" {} ''
    mkdir -p $out
    cp ${../console/physical.py} $out/physical.py
    cp ${../console/physical_viewer.py} $out/physical_viewer.py
    cp ${../console/screen.py} $out/screen.py
  '';
  physical = pkgs.writeShellApplication {
    name = "anima-physical-console";
    runtimeInputs = [ python pkgs.tigervnc pkgs.xset ];
    text = ''
      export QT_PLUGIN_PATH="${pkgs.qt6.qtbase}/${pkgs.qt6.qtbase.qtPluginPrefix}"
      exec python3 ${physicalSource}/physical.py
    '';
  };
  startDisplay = pkgs.writeShellScript "anima-shared-display" ''
    set -eu
    umask 0077
    touch /run/anima-console/Xauthority
    ${pkgs.xauth}/bin/xauth -f /run/anima-console/Xauthority add :99 . "$(${pkgs.util-linux}/bin/mcookie)"
    exec ${pkgs.tigervnc}/bin/Xvnc :99 \
      -auth /run/anima-console/Xauthority -nolisten tcp \
      -geometry 1600x900 -depth 24 -rfbport -1 \
      -rfbunixpath /run/anima-console/framebuffer.sock -rfbunixmode 0660 \
      -SecurityTypes None -AlwaysShared -DisconnectClients=0 \
      -AcceptSetDesktopSize=1 -FrameRate=30 -AcceptCutText=0 -SendCutText=0 -AllowOverride=
  '';
  startConsole = pkgs.writeShellScript "anima-shared-session" ''
    set -eu
    for attempt in $(${pkgs.coreutils}/bin/seq 1 100); do
      if ${pkgs.xdpyinfo}/bin/xdpyinfo -display :99 >/dev/null 2>&1; then
        ${pkgs.openbox}/bin/openbox --config-file /etc/anima/shared-desktop.xml &
        exec /run/current-system/sw/bin/anima-console
      fi
      sleep 0.1
    done
    exit 1
  '';
  # Only the shared desktop grabs the shortcut. The physical viewer must forward it.
  desktopConfig = pkgs.writeText "anima-shared-desktop.xml" ''
    <openbox_config xmlns="http://openbox.org/3.4/rc">
      <applications><application class="*"><decor>no</decor></application></applications>
      <keyboard><keybind key="C-A-l"><action name="Execute"><command>${pkgs.dbus}/bin/dbus-send --session --type=method_call --dest=ing.anima.Console /ing/anima/Console ing.anima.Console.Lock</command></action></keybind></keyboard>
      <mouse/><menu/>
    </openbox_config>
  '';
in {
  imports = [ ./vnc-identity.nix ];
  environment.etc."anima/shared-desktop.xml".source = desktopConfig;
  environment.etc."anima/shared-console".text = "1\n";
  environment.systemPackages = [ physical pkgs.tigervnc ];
  users.groups.anima-console = {};
  users.users.admin.extraGroups = [ "anima-console" ];
  users.users.xrdp.extraGroups = [ "anima-console" ];
  systemd.services.anima-shared-display = {
    description = "Persistent headless Anima console display";
    wants = [ "anima-shared-session.service" "anima-vnc.service" ];
    wantedBy = [ "multi-user.target" ];
    after = [ "systemd-tmpfiles-setup.service" ];
    serviceConfig = {
      User = "admin"; Group = "anima-console";
      RuntimeDirectory = "anima-console"; RuntimeDirectoryMode = "0750";
      ExecStart = startDisplay;
      Restart = "always"; RestartSec = 1;
      UMask = "0077";
    };
  };
  systemd.services.anima-shared-session = {
    description = "One Anima console for local and remote viewers";
    wantedBy = [ "multi-user.target" ];
    requires = [ "anima-shared-display.service" ];
    after = [ "anima-shared-display.service" ];
    bindsTo = [ "anima-shared-display.service" ];
    partOf = [ "anima-shared-display.service" ];
    environment = {
      DISPLAY = ":99";
      XAUTHORITY = "/run/anima-console/Xauthority";
      ANIMA_SHARED_CONSOLE = "1";
      # The persistent Xvnc display must work without a hardware GLX context.
      QTWEBENGINE_CHROMIUM_FLAGS = "--disable-gpu";
      HOME = "/home/admin";
    };
    serviceConfig = {
      User = "admin"; Group = "users";
      ExecStart = "${pkgs.dbus}/bin/dbus-run-session --dbus-daemon=${pkgs.dbus}/bin/dbus-daemon -- ${startConsole}";
      Restart = "always"; RestartSec = 1;
    };
  };
  security.pam.services.anima-vnc = {
    allowNullPassword = false;
    unixAuth = true;
  };
  systemd.services.anima-vnc = {
    description = "Authenticated private access to the shared console";
    wantedBy = [ "multi-user.target" ];
    requires = [ "anima-shared-session.service" "anima-vnc-identity.service" ];
    after = [ "anima-shared-session.service" "anima-vnc-identity.service" ];
    partOf = [ "anima-shared-display.service" ];
    environment = { DISPLAY = ":99"; XAUTHORITY = "/run/anima-console/Xauthority"; };
    serviceConfig = {
      User = "admin"; Group = "users";
      LoadCredential = [ "cert.pem:/persist/vnc/identity/cert.pem" "key.pem:/persist/vnc/identity/key.pem" ];
      ExecStart = "${pkgs.tigervnc}/bin/x0vncserver -display :99 -localhost -rfbport 5900 -SecurityTypes X509Plain -X509Cert=%d/cert.pem -X509Key=%d/key.pem -FrameRate=30 -PAMService anima-vnc -PlainUsers admin -AlwaysShared -DisconnectClients=0 -AcceptSetDesktopSize=0 -AcceptCutText=0 -SendCutText=0";
      Restart = "always"; RestartSec = 2;
      KillMode = "control-group";
    };
  };
}
