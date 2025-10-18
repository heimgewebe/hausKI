# Local Development Runbook

Follow this sequence to bring up the Hauski core locally.

## 1. Bootstrap

Prepare local configuration once per machine:

```bash
bash scripts/bootstrap.sh
```

This creates `configs/hauski.yml` with sane defaults and directories for data and models.

## 2. Build

Compile the workspace to ensure all crates compile together:

```bash
just build
```

## 3. Serve

Start the core service locally on port 8080. Keep this terminal open while developing:

```bash
just run-core
```

This invokes `cargo run -p hauski-cli -- serve` under the hood. If you need to expose configuration for debugging, you can alternatively run `just run-core-expose`.

## 4. Health check

In a second terminal, verify the service is healthy:

```bash
curl http://localhost:8080/healthz
curl -s -X POST http://localhost:8080/v1/chat -H 'Content-Type: application/json' -d '{"messages":[{"role":"user","content":"ping"}]}'
```

A successful health probe returns `ok`. The chat stub currently responds with `501 Not Implemented`, confirming the route is wired. Stop the server with `Ctrl+C` when you're done.

