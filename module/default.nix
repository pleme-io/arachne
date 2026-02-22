# Arachne home-manager module — daemon service
#
# Namespace: services.arachne.daemon.*
#
# Runs `arachne serve` as a persistent service for local development.
#
# Module factory: receives { hmHelpers } from flake.nix, returns HM module.
{ hmHelpers }:
{
  lib,
  config,
  pkgs,
  ...
}:
with lib; let
  inherit (hmHelpers) mkLaunchdService mkSystemdService;
  cfg = config.services.arachne.daemon;
  isDarwin = pkgs.stdenv.isDarwin;
in {
  options.services.arachne.daemon = {
    enable = mkOption {
      type = types.bool;
      default = false;
      description = "Enable Arachne classifieds scraper service";
    };

    package = mkOption {
      type = types.package;
      default = pkgs.arachne;
      description = "Arachne package";
    };

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

  config = let
    env = {
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
  in mkMerge [
    (mkIf (cfg.enable && isDarwin)
      (mkLaunchdService {
        name = "arachne";
        label = "io.pleme.arachne";
        command = "${cfg.package}/bin/arachne";
        args = ["serve" "--port" (toString cfg.port)];
        inherit env;
        logDir = "${config.home.homeDirectory}/Library/Logs";
      }))

    (mkIf (cfg.enable && !isDarwin)
      (mkSystemdService {
        name = "arachne";
        description = "Arachne classifieds scraper service";
        command = "${cfg.package}/bin/arachne";
        args = ["serve" "--port" (toString cfg.port)];
        inherit env;
      }))
  ];
}
