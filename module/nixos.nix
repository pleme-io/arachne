# Arachne NixOS module — system-level service
#
# Namespace: services.arachne.*
{ config, lib, pkgs, ... }:
with lib; let
  cfg = config.services.arachne;
in {
  options.services.arachne = {
    enable = mkEnableOption "Arachne classifieds scraper service";

    package = mkPackageOption pkgs "arachne" {};

    port = mkOption {
      type = types.int;
      default = 8080;
      description = "Health server listen port";
    };

    databaseUrl = mkOption {
      type = types.nullOr types.str;
      default = null;
      description = "PostgreSQL connection URL for persistence";
    };

    redisUrl = mkOption {
      type = types.nullOr types.str;
      default = null;
      description = "Redis connection URL";
    };

    chromeWsUrl = mkOption {
      type = types.str;
      default = "ws://localhost:9222";
      description = "Chrome DevTools Protocol WebSocket URL";
    };

    s3 = {
      endpoint = mkOption {
        type = types.nullOr types.str;
        default = null;
        description = "S3-compatible endpoint URL for photo storage";
      };

      bucket = mkOption {
        type = types.str;
        default = "arachne-photos";
        description = "S3 bucket name for photos";
      };

      region = mkOption {
        type = types.str;
        default = "us-east-1";
        description = "S3 region";
      };
    };

    logLevel = mkOption {
      type = types.str;
      default = "arachne=info";
      description = "RUST_LOG filter string";
    };

    extraEnv = mkOption {
      type = types.attrsOf types.str;
      default = {};
      description = "Additional environment variables";
    };
  };

  config = mkIf cfg.enable {
    systemd.services.arachne = {
      description = "Arachne classifieds scraper service";
      after = ["network.target"];
      wantedBy = ["multi-user.target"];
      serviceConfig = {
        ExecStart = "${cfg.package}/bin/arachne serve --port ${toString cfg.port}";
        DynamicUser = true;
        Restart = "on-failure";
        RestartSec = 5;
        ProtectSystem = "strict";
        ProtectHome = true;
        NoNewPrivileges = true;
      };
      environment = {
        RUST_LOG = cfg.logLevel;
        PORT = toString cfg.port;
        CHROME_WS_URL = cfg.chromeWsUrl;
        S3_BUCKET = cfg.s3.bucket;
        S3_REGION = cfg.s3.region;
      }
      // optionalAttrs (cfg.databaseUrl != null) { DATABASE_URL = cfg.databaseUrl; }
      // optionalAttrs (cfg.redisUrl != null) { REDIS_URL = cfg.redisUrl; }
      // optionalAttrs (cfg.s3.endpoint != null) { S3_ENDPOINT = cfg.s3.endpoint; }
      // cfg.extraEnv;
    };
  };
}
