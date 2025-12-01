# utoipa-swagger-ui-vendored

This crate provides a minimal vendored Swagger UI archive used by `utoipa-swagger-ui` when the `vendored` feature is enabled. The bundled assets originate from the Swagger UI project (Apache-2.0 license). Keeping the assets locally avoids network fetches during offline builds.

The ZIP archive is embedded directly in `src/lib.rs` as a byte slice to avoid committing binary files. If you update the upstream Swagger UI version, regenerate the inline byte array using the instructions in `src/lib.rs` and ensure the license information remains present.
