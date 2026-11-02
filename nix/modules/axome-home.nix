{ self }:
{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.programs.axome;
  settingsDir = "${config.xdg.configHome}/axome";
in
{
  options.programs.axome = {
    enable = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = "Whether to install Axome when this module is imported.";
    };

    package = lib.mkOption {
      type = lib.types.package;
      default = self.packages.${pkgs.stdenv.hostPlatform.system}.axome;
      defaultText = lib.literalExpression "self.packages.<system>.axome";
      description = "Axome desktop package to install.";
    };

    settings = lib.mkOption {
      type = lib.types.attrsOf (pkgs.formats.json { }).type;
      default = { };
      description = "Global Axome settings. Stored in the Nix store; do not put secrets here.";
    };

    identityFile = lib.mkOption {
      type = lib.types.nullOr (lib.types.strMatching "/.+");
      default = null;
      example = lib.literalExpression "config.sops.secrets.axome-identity.path";
      description = ''
        Provisioned desktop identity (from `axome-server keygen`), linked to
        `~/.config/axome/client.pem` like an SSH key in `~/.ssh`. Keeps this desktop's fingerprint
        stable so servers can list it in `authorizedKeys`. Must be mode 0600 or stricter and live
        outside the Nix store. Unset means Axome generates one on first use.
      '';
    };
  };

  config = lib.mkIf cfg.enable {
    home.packages = [ cfg.package ];

    assertions = [
      {
        assertion = cfg.identityFile == null || !lib.hasPrefix builtins.storeDir cfg.identityFile;
        message = "programs.axome.identityFile must not be a Nix store path; the store is world-readable.";
      }
    ];

    xdg.configFile = lib.mkMerge [
      (lib.mkIf (cfg.identityFile != null) {
        "axome/client.pem".source = config.lib.file.mkOutOfStoreSymlink cfg.identityFile;
      })
      (lib.mkIf (cfg.settings != { }) {
        "axome/settings_source" = {
          text = builtins.toJSON cfg.settings;
          onChange = ''
            coreutils="${pkgs.coreutils}/bin"
            settings_dir=${lib.escapeShellArg settingsDir}
            "$coreutils/mkdir" -p "$settings_dir"
            "$coreutils/chmod" 700 "$settings_dir"
            tmp_file="$("$coreutils/mktemp" "$settings_dir/.settings.jsonc.XXXXXX")"
            "$coreutils/cp" "$settings_dir/settings_source" "$tmp_file"
            "$coreutils/chmod" 600 "$tmp_file"
            "$coreutils/mv" -f "$tmp_file" "$settings_dir/settings.jsonc"
          '';
        };
      })
    ];
  };
}
