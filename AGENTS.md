# Policy

- Prioritize reducing functions, structures, traits, and dependencies (cargo add and cargo:rustc-link-lib= in build.rs) over increasing them
- Prioritize requiring minimal effort over misleading the user when deciding between them
- Prioritize STEP file specification over OCCT specification when in doubt

# Agent

- For implementation instructions, run tests to verify after implementation
- run "cargo fmt" to all new changes.
- Remove comments that the code already says and keep a comment within two lines.