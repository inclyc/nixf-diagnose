{
  lib,
  rustPlatform,
  nixf,
}:

rustPlatform.buildRustPackage (finalAttrs: {
  pname = "nixf-diagnose";
  version = "nightly";

  src = ./.;

  env.NIXF_TIDY_PATH = lib.getExe nixf;

  useFetchCargoVendor = true;
  cargoHash = "sha256-LutCktLHpfl5aMvN9RW0IL9nojcq4j2kjc9zfeePCMg=";

  meta = {
    description = "CLI wrapper for nixf-tidy with fancy diagnostic output";
    mainProgram = "nixf-diagnose";
    homepage = "https://github.com/inclyc/nixf-diagnose";
    license = lib.licenses.mit;
    maintainers = with lib.maintainers; [ inclyc ];
  };
})
