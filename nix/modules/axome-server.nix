{ self }:
{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.services.axome-server;
  home = config.users.users.${cfg.user}.home;
  portOf = address: lib.toInt (lib.last (lib.splitString ":" address));
  authorizedKeysFile = pkgs.writeText "axome-authorized-keys" (
    lib.concatMapStrings (key: "${key}\n") cfg.authorizedKeys
  );
  # Holds only addresses and file paths, never a private key.
  serverConfig = (pkgs.formats.json { }).generate "axome-server.jsonc" (
    {
      inherit (cfg) listen;
    }
    // lib.optionalAttrs (cfg.identityFile != null) { identity_file = cfg.identityFile; }
    // lib.optionalAttrs (cfg.authorizedKeys != [ ]) {
      authorized_keys_file = "${authorizedKeysFile}";
    }
    // lib.optionalAttrs cfg.web.enable { web.listen = cfg.web.listen; }
  );
  # The service and shell commands (`pair`, `fingerprint`) must agree on the
  # identity and port, so both run through this.
  cli = pkgs.writeShellScriptBin "axome-server" ''
    exec ${cfg.package}/bin/axome-server --config ${serverConfig} "$@"
  '';
in
{
  options.services.axome-server = {
    enable = lib.mkEnableOption "Axome remote workspace server";

    package = lib.mkOption {
      type = lib.types.package;
      default = self.packages.${pkgs.stdenv.hostPlatform.system}.axome-server;
      defaultText = lib.literalExpression "self.packages.<system>.axome-server";
      description = "Axome server package to use.";
    };

    user = lib.mkOption {
      type = lib.types.strMatching "[a-z_][a-z0-9_-]*[$]?";
      description = "Existing user account that owns the server identity and data.";
    };

    listen = lib.mkOption {
      type = lib.types.str;
      default = "0.0.0.0:7420";
      description = "UDP address and port on which the Axome server listens.";
    };

    identityFile = lib.mkOption {
      type = lib.types.nullOr (lib.types.strMatching "/.+");
      default = null;
      example = "/run/secrets/axome-server-identity";
      description = ''
        Provisioned server identity (from `axome-server keygen`), like openssh's host keys: keeps
        the fingerprint desktops pin stable across reinstalls. Must be owned by `user` with mode
        0600 or stricter and live outside the Nix store (e.g. sops-nix/agenix). Unset means one
        is generated in the user's config directory on first start.
      '';
    };

    authorizedKeys = lib.mkOption {
      type = lib.types.listOf (lib.types.strMatching "SHA256:[0-9a-fA-F]{64}( .*)?");
      default = [ ];
      example = [ "SHA256:0123…cdef laptop" ];
      description = ''
        Client fingerprints admitted without pairing, as `SHA256:<hex> [name]`. Fingerprints are
        public, so this file lives in the Nix store. An unpaired client's fingerprint is logged
        when it connects.
      '';
    };

    xdgConfigHome = lib.mkOption {
      type = lib.types.nullOr (lib.types.strMatching "/.+");
      default = null;
      description = "XDG config home for the server; use the same path as the user's shell when customized.";
    };

    xdgDataHome = lib.mkOption {
      type = lib.types.nullOr (lib.types.strMatching "/.+");
      default = null;
      description = "XDG data home for the server; use the same path as the user's shell when customized.";
    };

    extraPackages = lib.mkOption {
      type = lib.types.listOf lib.types.package;
      default = [ ];
      description = "Additional commands available to server-side workspaces and tasks.";
    };

    openFirewall = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = "Whether to open the configured UDP port in the firewall.";
    };

    web = {
      enable = lib.mkEnableOption ''
        the Axome web frontend. It has no authentication and grants the service user's full
        filesystem and process authority to anyone who can reach it; keep it on loopback or
        behind an authenticating proxy/VPN
      '';

      listen = lib.mkOption {
        type = lib.types.str;
        default = "127.0.0.1:7421";
        description = "TCP address and port for the web frontend.";
      };

      openFirewall = lib.mkOption {
        type = lib.types.bool;
        default = false;
        description = "Whether to open the web TCP port in the firewall.";
      };
    };
  };

  config = lib.mkIf cfg.enable {
    assertions = [
      {
        assertion = builtins.hasAttr cfg.user config.users.users;
        message = "services.axome-server.user must name a configured NixOS user.";
      }
      {
        assertion = cfg.identityFile == null || !lib.hasPrefix builtins.storeDir cfg.identityFile;
        message = "services.axome-server.identityFile must not be a Nix store path; the store is world-readable.";
      }
    ];
    environment.systemPackages = [ cli ];
    networking.firewall.allowedUDPPorts = lib.mkIf cfg.openFirewall [ (portOf cfg.listen) ];
    networking.firewall.allowedTCPPorts = lib.mkIf (cfg.web.enable && cfg.web.openFirewall) [
      (portOf cfg.web.listen)
    ];

    systemd.services.axome-server = {
      description = "Axome remote workspace server";
      wantedBy = [ "multi-user.target" ];
      after = [ "network.target" ];
      path = [
        pkgs.git
        pkgs.nix
        pkgs.openssh
        pkgs.fish
        pkgs.bash
        pkgs.coreutils
      ]
      ++ cfg.extraPackages;
      environment = {
        HOME = home;
      }
      // lib.optionalAttrs (cfg.xdgConfigHome != null) {
        XDG_CONFIG_HOME = cfg.xdgConfigHome;
      }
      // lib.optionalAttrs (cfg.xdgDataHome != null) {
        XDG_DATA_HOME = cfg.xdgDataHome;
      };
      serviceConfig = {
        User = cfg.user;
        WorkingDirectory = home;
        ExecStart = "${cli}/bin/axome-server serve";
        Restart = "on-failure";
        KillMode = "mixed";
      };
    };
  };
}
