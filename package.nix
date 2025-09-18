{
  lib,
  rustPlatform,
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

  doCheck = true;

  meta = with lib; {
    description = "A command-line tool for building and deploying NixOS configurations locally and remotely";
    maintainers = with maintainers; [ tfc ];
    license = licenses.gpl3;
    platforms = platforms.linux ++ platforms.darwin;
    mainProgram = "nxbd";
  };
}
