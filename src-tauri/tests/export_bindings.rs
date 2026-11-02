//! `cargo test -p openclion --test export_bindings` regenerates `src/bindings.ts`
//! without launching the app (used by `pnpm gen:bindings` and CI drift checks).

#[test]
fn export_bindings() {
    openclion_lib::export_bindings(&openclion_lib::specta_builder());
}
