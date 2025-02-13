{
  lib,
  stdenv,
  rustPlatform,
  darwin,
}:

rustPlatform.buildRustPackage {
  name = "nxbd";

  src = lib.fileset.toSource {
    root = ./.;
    fileset = lib.fileset.unions [
      ./src
      ./Cargo.lock
      ./Cargo.toml
    ];
  };

  strictDeps = true;

  cargoLock.lockFile = ./Cargo.lock;

  buildInputs = lib.optionals stdenv.isDarwin [
    darwin.apple_sdk.frameworks.CoreFoundation
    darwin.apple_sdk.frameworks.Security
    darwin.apple_sdk.frameworks.IOKit
    darwin.libiconv
  ];

  doCheck = true;

  meta = with lib; {
    description = "A command-line tool for building and deploying NixOS configurations locally and remotely";
    maintainers = with maintainers; [ tfc ];
    license = licenses.gpl3;
    platforms = platforms.linux ++ platforms.darwin;
    mainProgram = "nxbd";
  };
}
