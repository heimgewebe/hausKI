#!/usr/bin/env python3
# Minimaler Ollama-Mock (Stdlib):
#  GET  /api/tags  -> {"models":[{"name":"llama3:8b"}]}
#  POST /api/chat  -> {"message":{"content":"(mock) <echo>" }}
import json, sys
from http.server import HTTPServer, BaseHTTPRequestHandler

HOST = "127.0.0.1"
PORT = 11434

class Handler(BaseHTTPRequestHandler):
    def _send(self, code:int, body:dict):
        data = json.dumps(body).encode("utf-8")
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def do_GET(self):
        if self.path.startswith("/api/tags"):
            self._send(200, {"models":[{"name":"llama3:8b"}]})
        else:
            self._send(404, {"error":"not found"})

    def do_POST(self):
        if self.path.startswith("/api/chat"):
            length = int(self.headers.get("Content-Length","0"))
            raw = self.rfile.read(length) if length > 0 else b"{}"
            try:
                payload = json.loads(raw.decode("utf-8"))
            except Exception:
                payload = {}
            content = "(mock) ok"
            for m in (payload.get("messages") or [])[::-1]:
                if m.get("role") == "user":
                    content = "(mock) " + str(m.get("content","")); break
            self._send(200, {"message":{"content":content}})
        else:
            self._send(404, {"error":"not found"})

def main():
    print(f"mock-ollama listening on http://{HOST}:{PORT}", file=sys.stderr)
    HTTPServer((HOST, PORT), Handler).serve_forever()

if __name__ == "__main__":
    main()
