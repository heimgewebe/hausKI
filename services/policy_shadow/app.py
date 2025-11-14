"""Minimal FastAPI service for HausKI policy experimentation."""

from __future__ import annotations

import json
import os
import platform
import uuid
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

from fastapi import Depends, FastAPI, Header, HTTPException, status
from pydantic import BaseModel, ConfigDict, Field

APP_VERSION = os.getenv("HAUSKI_VERSION", "0.1.0")
TOKEN_ENV = "HAUSKI_TOKEN"
EVENT_BASE_ENV = "HAUSKI_DATA"


def _event_dir() -> Path:
    base = Path(os.getenv(EVENT_BASE_ENV, Path.home() / ".hauski"))
    events_dir = base / "events"
    events_dir.mkdir(parents=True, exist_ok=True)
    return events_dir


def append_event(kind: str, payload: dict[str, Any]) -> None:
    """Append an event line using the shared HausKI schema."""
    events_dir = _event_dir()
    now = datetime.now(timezone.utc)
    filename = events_dir / f"{now.astimezone().strftime('%Y-%m')}.jsonl"

    node_id = os.getenv("HAUSKI_NODE_ID") or platform.node() or "unknown"
    event_line = {
        "id": os.getenv("HAUSKI_EVENT_ID", str(uuid.uuid4())),
        "node_id": node_id,
        "ts": int(now.timestamp() * 1000),
        "kind": kind,
        "payload": payload,
    }

    with filename.open("a", encoding="utf-8") as handle:
        handle.write(json.dumps(event_line, ensure_ascii=False))
        handle.write("\n")


class MetricsIngest(BaseModel):
    model_config = ConfigDict(extra="allow")

    ts: int = Field(..., description="Client-side timestamp in milliseconds")
    host: str = Field(..., description="Hostname emitting the metrics")
    updates: dict[str, Any]
    backup: dict[str, Any]
    drift: dict[str, Any]


class PolicyDecisionRequest(BaseModel):
    model_config = ConfigDict(extra="allow")

    ts: int | None = Field(None, description="Client timestamp in milliseconds")
    context: dict[str, Any] = Field(default_factory=dict)


class PolicyDecisionResponse(BaseModel):
    action: str
    score: float
    why: str
    context: dict[str, Any]


class PolicyFeedback(BaseModel):
    model_config = ConfigDict(extra="allow")

    decision_id: str | None = None
    rating: float | None = None
    notes: str | None = None
    metadata: dict[str, Any] = Field(default_factory=dict)


app = FastAPI(title="HausKI Shadow Policy API", version=APP_VERSION)

_latest_metrics: dict[str, Any] = {}


def require_token(x_auth: str | None = Header(default=None)) -> None:
    expected = os.getenv(TOKEN_ENV)
    if expected and x_auth != expected:
        raise HTTPException(
            status_code=status.HTTP_401_UNAUTHORIZED,
            detail="invalid or missing x-auth token",
        )


@app.post("/v1/ingest/metrics", dependencies=[Depends(require_token)])
def ingest_metrics(payload: MetricsIngest) -> dict[str, str]:
    global _latest_metrics
    _latest_metrics = payload.model_dump()
    append_event("metrics.ingest", _latest_metrics)
    return {"status": "ok"}


@app.post(
    "/v1/policy/decide",
    dependencies=[Depends(require_token)],
    response_model=PolicyDecisionResponse,
)
def policy_decide(request: PolicyDecisionRequest) -> PolicyDecisionResponse:
    now = datetime.now().astimezone()
    hour = now.hour
    if hour < 12:
        action = "remind.morning"
        why = "Morning shadow policy recommendation"
    elif hour < 18:
        action = "remind.afternoon"
        why = "Afternoon shadow policy recommendation"
    else:
        action = "remind.evening"
        why = "Evening shadow policy recommendation"

    response = PolicyDecisionResponse(
        action=action,
        score=0.5,
        why=why,
        context={"requested": request.context, "observed_hour": hour},
    )

    append_event("policy.shadow_decide", response.model_dump())
    return response


@app.post("/v1/policy/feedback", dependencies=[Depends(require_token)])
def policy_feedback(feedback: PolicyFeedback) -> dict[str, str]:
    append_event("policy.feedback", feedback.model_dump())
    return {"status": "queued"}


@app.get("/v1/health/latest", dependencies=[Depends(require_token)])
def health_latest() -> dict[str, Any]:
    return {"status": "ok", "metrics": _latest_metrics or None}


@app.get("/version")
def version() -> dict[str, str]:
    return {"version": APP_VERSION}
