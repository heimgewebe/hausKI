# hausKI API Dokumentation

Automatisch generiert mit [utoipa](https://docs.rs/utoipa) und [Swagger UI](https://swagger.io/tools/swagger-ui/).

**Endpoints:**  
- `GET /docs` – Swagger UI  
- `GET /api-docs/openapi.json` – maschinelles Schema

```bash
curl -sS http://127.0.0.1:8080/api-docs/openapi.json | jq '.info.title'
```

Der Legacy-Pfad `/docs/openapi.json` leitet per 308-Redirect auf den neuen
Kanal um.
- `GET /docs/openapi.json` – 308-Redirect auf `/api-docs/openapi.json`
