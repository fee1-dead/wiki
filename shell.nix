with import <nixpkgs> {};
stdenv.mkDerivation {
  name = "wiki-env";
  buildInputs = [ pkg-config openssl gcc ];
}