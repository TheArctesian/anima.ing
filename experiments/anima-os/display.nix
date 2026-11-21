{ pkgs, lib, workspaceRoot ? ../.., ... }:
let
  session = import ./session-package.nix { inherit pkgs workspaceRoot; };
  display = pkgs.writeShellApplication {
    name = "anima-display";
    runtimeInputs = with pkgs; [ xrandr xinput systemd util-linux coreutils ];
    text = ''
      case "''${1:-}" in
        --auto) exec ${session}/bin/anima-os-session display-auto ;;
        --list) exec ${session}/bin/anima-os-session display-list ;;
        --set) exec ${session}/bin/anima-os-session display-set "''${2:-}" "''${3:-}" ;;
        --keep) exec ${session}/bin/anima-os-session display-keep "''${2:-}" ;;
        --revert) exec ${session}/bin/anima-os-session display-revert ;;
        --save) exec ${session}/bin/anima-os-session display-save "''${2:-}" ;;
        *) echo "Open Settings → Display in the graphical console." >&2; exit 1 ;;
      esac
    '';
  };
in {
  environment.systemPackages = [ display ];
  services.xserver.displayManager.sessionCommands = lib.mkBefore ''
    ${display}/bin/anima-display --auto || true
  '';
}
