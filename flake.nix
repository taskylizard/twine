{
  description = "twine dev shell";

  inputs = {
    nixpkgs.url = "https://channels.nixos.org/nixpkgs-unstable/nixexprs.tar.xz";
  };

  outputs =
    { nixpkgs, ... }:
    let
      system = "x86_64-linux";
      pkgs = import nixpkgs { inherit system; };
    in
    {
      devShells.${system}.default = pkgs.mkShell {
        buildInputs = with pkgs; [
          rocksdb
          snappy
        ];

        nativeBuildInputs = with pkgs; [
          pkg-config
          clang
        ];

        ROCKSDB_LIB_DIR = "${pkgs.rocksdb}/lib";
        ROCKSDB_INCLUDE_DIR = "${pkgs.rocksdb}/include";
      };
    };
}
