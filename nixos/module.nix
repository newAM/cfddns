{
  config,
  lib,
  pkgs,
  ...
}: let
  cfg = config.services.cfddns;
  settingsFormat = pkgs.formats.json {};
  configurationFile = settingsFormat.generate "cfddns_config.json" cfg.settings;
in {
  options.services.cfddns = {
    enable = lib.mkEnableOption "Cloudflare Dynamic DNS";

    settings = lib.mkOption {
      type = lib.types.submodule {
        freeformType = lib.types.attrsOf settingsFormat.type;

        options = {
          a_interface = lib.mkOption {
            default = null;
            description = ''
              Network interface to obtain IPv4 from.

              Takes priority over {option}`services.cfddns.settings.a_http` if
              non-null.
            '';
            example = "bond-wan";
            type = lib.types.nullOr lib.types.str;
          };

          a_http = lib.mkOption {
            default = null;
            description = "URL that returns an IPv4 from an HTTP GET.";
            example = "https://ipv4.icanhazip.com";
            type = lib.types.nullOr lib.types.str;
          };

          aaaa_interface = lib.mkOption {
            default = null;
            description = "Network interface to obtain IPv6 prefix.";
            example = "br-lan";
            type = lib.types.nullOr lib.types.str;
          };

          aaaa_http = lib.mkOption {
            default = null;
            description = "URL that returns an IPv6 from an HTTP GET.";
            example = "https://ipv6.icanhazip.com";
            type = lib.types.nullOr lib.types.str;
          };

          zones = lib.mkOption {
            default = [];
            type = lib.types.listOf (lib.types.submodule {
              freeformType = lib.types.attrsOf settingsFormat.type;
              options = {
                name = lib.mkOption {
                  description = "Zone name";
                  example = "mydomain.com";
                  type = lib.types.str;
                };
                records = lib.mkOption {
                  default = [];
                  type = lib.types.listOf (lib.types.submodule {
                    freeformType = lib.types.attrsOf settingsFormat.type;
                    options = {
                      name = lib.mkOption {
                        description = "Record name";
                        example = "mysubdomain";
                        type = lib.types.str;
                      };
                      ttl = lib.mkOption {
                        description = "Record TTL";
                        default = null;
                        type = lib.types.nullOr lib.types.int;
                      };
                      proxied = lib.mkOption {
                        description = "Record proxy status";
                        default = null;
                        type = lib.types.nullOr lib.types.bool;
                      };
                      suffix = lib.mkOption {
                        default = null;
                        description = ''
                          Record IPv6 suffix.

                          IPv6 updates are skipped if null.
                        '';
                        example = "::aaaa:aaaa:aaaa:aaaa";
                        type = lib.types.nullOr lib.types.str;
                      };
                    };
                  });
                };
              };
            });
          };

          history_path = lib.mkOption {
            default = "/var/lib/cfddns/history.json";
            description = "History file for retaining previous IPs";
            type = lib.types.str;
          };

          log_level = lib.mkOption {
            default = "info";
            description = "Logging level.";
            type = lib.types.enum [
              "off"
              "error"
              "warn"
              "info"
              "debug"
              "trace"
            ];
          };
        };
      };
    };

    environmentFiles = lib.mkOption {
      type = lib.types.listOf lib.types.path;
      description = ''
        Environment file as defined in {manpage}`systemd.exec(5)`.

        cfddns uses the following environment variables for passing secrets:

        * `CLOUDFLARE_TOKEN`: Cloudflare API token

        Example contents:

        ```
        CLOUDFLARE_TOKEN=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
        ```
      '';
      example = ["/run/keys/cfddns.env"];
    };

    startAt = lib.mkOption {
      type = lib.types.str;
      default = "*:0/10";
      example = "hourly";
      description = ''
        How often to run the dynamic DNS updater.

        The format is described in {manpage}`systemd.time(7)`.
      '';
    };
  };

  config = lib.mkIf cfg.enable {
    systemd.services.cfddns = {
      wants = ["network-online.target"];
      after = ["network-online.target"];

      inherit (cfg) startAt;

      description = "Cloudflare dynamic DNS";

      serviceConfig = {
        Type = "idle";
        KillSignal = "SIGINT";
        ExecStart = "${pkgs.cfddns}/bin/cfddns ${configurationFile}";
        EnvironmentFile = cfg.environmentFiles;

        # hardening
        StateDirectory = "cfddns";
        StateDirectoryMode = "0700";
        DynamicUser = true;
        DevicePolicy = "closed";
        CapabilityBoundingSet = "";
        RestrictAddressFamilies = [
          "AF_INET"
          "AF_INET6"
          "AF_UNIX"
          "AF_NETLINK"
        ];
        DeviceAllow = "";
        NoNewPrivileges = true;
        PrivateDevices = true;
        PrivateMounts = true;
        PrivateTmp = true;
        PrivateUsers = true;
        ProtectClock = true;
        ProtectControlGroups = true;
        ProtectHome = true;
        ProtectKernelLogs = true;
        ProtectKernelModules = true;
        ProtectKernelTunables = true;
        ProtectSystem = "strict";
        MemoryDenyWriteExecute = true;
        LockPersonality = true;
        RemoveIPC = true;
        RestrictNamespaces = true;
        RestrictRealtime = true;
        RestrictSUIDSGID = true;
        SystemCallArchitectures = "native";
        SystemCallFilter = [
          "@system-service"
          "~@privileged"
          "~@resources"
        ];
        ProtectProc = "invisible";
        ProtectHostname = true;
        ProcSubset = "pid";
        UMask = "0077";
      };
    };
  };

  meta.maintainers = pkgs.cfddns.meta.maintainers;
}
