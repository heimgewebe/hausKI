# Troubleshooting

## Port already in use

The core serves HTTP on port 8080. If you see `address already in use` errors when running `just run-core`, find and stop the conflicting process:

```bash
lsof -i :8080
kill <pid>
```

Alternatively, temporarily change the `server.port` value in `configs/hauski.yml`.

## GPU VRAM pressure

If the GPU runs out of VRAM while loading models, try the following:

- Use quantized models with lower memory requirements.
- Reduce concurrent requests to the service.
- Restart the process to release any leaked allocations.

## ASR models missing

If speech-recognition models are unavailable, pull them before starting the service:

```bash
just run-cli -- models pull <model-id>
```

Replace `<model-id>` with the identifier you need (for example `whisper-base.en`).

## Secret-scanning noise from vendored fixtures

GitHub Advanced Security may flag sample keys or credentials that live in vendored
dependencies under `vendor/`. We keep the vendor tree intact—deleting files would
invalidate Cargo's `.cargo-checksum.json` and break builds. Instead, the
configuration in `.github/secret_scanning.yml` ignores common fixture locations in
vendored crates. If you encounter alerts for these paths, confirm the files are
vendored test assets and close the alerts as "ignored by configuration". Never
remove files from `vendor/`; adjust the ignore list if additional fixture paths
are needed.

