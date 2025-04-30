{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    fenix = {
      url = "github:nix-community/fenix/monthly";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    naersk = {
      url = "github:nix-community/naersk";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, fenix, flake-utils, naersk, nixpkgs }:
    (flake-utils.lib.eachSystem [ "x86_64-linux" "aarch64-linux" ] (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        deps = with pkgs; [
          xorg.libX11
        ];

        devToolchain = fenix.packages.${system}.stable;

        lefthk = ((naersk.lib.${system}.override {
          inherit (fenix.packages.${system}.minimal) cargo rustc;
        }).buildPackage {
          name = "lefthk";
          src = ./.;
          buildInputs = deps;
          postFixup = ''
            for p in $out/bin/left*; do
              patchelf --set-rpath "${pkgs.lib.makeLibraryPath deps}" $p
            done
          '';
        });
      in
      rec {
        # `nix build`
        packages.lefthk = lefthk;
        defaultPackage = packages.lefthk;

        # `nix run`
        apps.lefthk = flake-utils.lib.mkApp {
          drv = packages.lefthk;
        };
        defaultApp = apps.lefthk;

        # `nix develop`
        devShell = pkgs.mkShell
          {
            buildInputs = deps ++ [ pkgs.pkg-config ];
            nativeBuildInputs = with pkgs; [
              (devToolchain.withComponents [
                "cargo"
                "clippy"
                "rust-src"
                "rustc"
                "rustfmt"
              ])
              fenix.packages.${system}.rust-analyzer
              xorg.xinit
            ];
          };
      })) // {
      overlay = final: prev: {
        lefthk = self.packages.${final.system}.lefthk;
      };
    };
}
