{ lib, pkgs, ... }:
let
  plain = {
    add_newline = true;
    command_timeout = 300;
    scan_timeout = 20;
    format = "ANIMA $username$hostname$directory$git_branch$git_status$cmd_duration$status$line_break> ";
    username = { show_always = true; format = "$user"; };
    hostname = { ssh_only = false; format = "@$hostname"; };
    directory = { format = " | $path"; truncation_length = 3; truncate_to_repo = false; };
    git_branch = { format = " | $branch"; };
    git_status = {
      format = "( $conflicted$stashed$deleted$renamed$modified$staged$untracked$ahead_behind)";
      conflicted = "=";
      stashed = "*";
      deleted = "x";
      renamed = "r";
      modified = "!";
      staged = "+";
      untracked = "?";
      ahead = ">";
      behind = "<";
      diverged = "<>";
    };
    cmd_duration = { min_time = 2000; format = " | $duration"; };
    status = { disabled = false; symbol = "exit "; success_symbol = "exit "; format = " | $symbol$status"; };
  };
  powerline = lib.recursiveUpdate plain {
    format = "[ ANIMA ](bg:#88C0D0 fg:#151C20)[](fg:#88C0D0 bg:#2E3440)$username$hostname[](fg:#2E3440 bg:#434C5E)$directory[](fg:#434C5E)$git_branch$git_status$cmd_duration$status$line_break$character";
    username = {
      style_user = "bg:#2E3440 fg:#ECEFF4";
      style_root = "bg:#2E3440 fg:#D99BA1 bold";
      format = "[ $user]($style)";
    };
    hostname = { format = "[@$hostname ]($style)"; style = "bg:#2E3440 fg:#ECEFF4"; };
    directory = { format = "[ $path ]($style)"; style = "bg:#434C5E fg:#ECEFF4"; };
    git_branch = { format = "[ $branch]($style)"; style = "#88C0D0"; };
    git_status = { format = "([ $all_status$ahead_behind]($style))"; style = "#D8B47B"; };
    cmd_duration = { format = "[ $duration]($style)"; style = "#A4A8B0"; };
    status = { format = "[ $symbol$status]($style)"; style = "#D99BA1"; success_style = "#88C0D0"; };
    character = { success_symbol = "[❯](#88C0D0)"; error_symbol = "[❯](#D99BA1)"; };
  };
in
{
  environment.systemPackages = [ pkgs.xterm ];
  environment.etc."anima/starship-powerline.toml".source =
    (pkgs.formats.toml {}).generate "anima-starship-powerline.toml" powerline;
  fonts.packages = [ pkgs.nerd-fonts.jetbrains-mono ];
  systemd.tmpfiles.rules = [ "d /run/anima-root-prompt 0700 root root -" ];
  programs.bash.interactiveShellInit = lib.mkBefore ''
    if [ "$EUID" -eq 0 ]; then
      export STARSHIP_CACHE=/run/anima-root-prompt
    fi
  '';
  programs.starship = {
    enable = true;
    interactiveOnly = true;
    settings = plain;
  };
}
