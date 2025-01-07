{
  description = "Development environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
        };
      in
      {
        formatter = pkgs.nixpkgs-fmt;
        devShells.default = pkgs.mkShell {
          packages = with pkgs; [
            influxdb2-server
            influxdb2-cli
            grafana
          ];

          shellHook = ''
            echo "Set up application with:"
            echo "  $ make setup"
            echo "Start applications with:"
            echo "  $ make start"
          '';
        };
      });
}
