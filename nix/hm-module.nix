# Home-manager module for Echo speech-to-text
#
# Provides a systemd user service for autostart.
# Usage: imports = [ echo.homeManagerModules.default ];
#        services.echo.enable = true;
{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.services.echo;
in
{
  options.services.echo = {
    enable = lib.mkEnableOption "Echo speech-to-text user service";

    package = lib.mkOption {
      type = lib.types.package;
      defaultText = lib.literalExpression "echo.packages.\${system}.echo";
      description = "The Echo package to use.";
    };
  };

  config = lib.mkIf cfg.enable {
    systemd.user.services.echo = {
      Unit = {
        Description = "Echo speech-to-text";
        After = [ "graphical-session.target" ];
        PartOf = [ "graphical-session.target" ];
      };
      Service = {
        ExecStart = "${cfg.package}/bin/echo";
        Restart = "on-failure";
        RestartSec = 5;
      };
      Install.WantedBy = [ "graphical-session.target" ];
    };
  };
}
