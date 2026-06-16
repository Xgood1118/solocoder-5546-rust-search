"""Unified Search Python SDK"""
from __future__ import annotations

import requests
from typing import Optional, List, Dict, Any
from dataclasses import dataclass, field


@dataclass
class FacetBucket:
    value: str
    count: int


@dataclass
class Facets:
    source: List[FacetBucket] = field(default_factory=list)
    author: List[FacetBucket] = field(default_factory=list)
    tags: List[FacetBucket] = field(default_factory=list)


@dataclass
class Document:
    source: str
    source_id: str
    title: str
    content: str
    url: Optional[str] = None
    author: Optional[str] = None
    created_at: Optional[str] = None
    updated_at: Optional[str] = None
    tags: List[str] = field(default_factory=list)
    project: Optional[str] = None


@dataclass
class SearchHit:
    doc: Document
    score: float
    snippet: Optional[str] = None


@dataclass
class SearchResponse:
    total: int
    hits: List[SearchHit]
    facets: Facets
    query: str
    elapsed_ms: int


class UnifiedSearchClient:
    def __init__(self, base_url: str = "http://localhost:8340"):
        self.base_url = base_url.rstrip("/")
        self.session = requests.Session()

    def search(
        self,
        q: str,
        *,
        limit: int = 20,
        offset: int = 0,
        source: Optional[List[str]] = None,
        author: Optional[List[str]] = None,
        tags: Optional[List[str]] = None,
        from_date: Optional[str] = None,
        to_date: Optional[str] = None,
    ) -> SearchResponse:
        body: Dict[str, Any] = {
            "q": q,
            "limit": limit,
            "offset": offset,
        }
        if source:
            body["source"] = source
        if author:
            body["author"] = author
        if tags:
            body["tags"] = tags
        if from_date:
            body["from_date"] = from_date
        if to_date:
            body["to_date"] = to_date

        resp = self.session.post(f"{self.base_url}/search", json=body)
        resp.raise_for_status()
        return self._parse_response(resp.json())

    def index_document(self, doc: Document) -> Dict[str, Any]:
        payload = {
            "source": doc.source,
            "source_id": doc.source_id,
            "title": doc.title,
            "content": doc.content,
            "created_at": doc.created_at or "2025-01-01T00:00:00Z",
            "updated_at": doc.updated_at or "2025-01-01T00:00:00Z",
            "tags": doc.tags,
        }
        if doc.url:
            payload["url"] = doc.url
        if doc.author:
            payload["author"] = doc.author
        if doc.project:
            payload["project"] = doc.project

        resp = self.session.post(f"{self.base_url}/documents", json=payload)
        resp.raise_for_status()
        return resp.json()

    def force_commit(self) -> Dict[str, Any]:
        resp = self.session.post(f"{self.base_url}/commit")
        resp.raise_for_status()
        return resp.json()

    def fetch_connector(
        self,
        source_type: str,
        params: Dict[str, Any],
        last_fetched_at: Optional[str] = None,
    ) -> Dict[str, Any]:
        body: Dict[str, Any] = {
            "source_type": source_type,
            "params": params,
        }
        if last_fetched_at:
            body["last_fetched_at"] = last_fetched_at

        resp = self.session.post(f"{self.base_url}/connectors/fetch", json=body)
        resp.raise_for_status()
        return resp.json()

    def list_connectors(self) -> Dict[str, Any]:
        resp = self.session.get(f"{self.base_url}/connectors")
        resp.raise_for_status()
        return resp.json()

    def _parse_response(self, data: Dict[str, Any]) -> SearchResponse:
        facets_data = data.get("facets", {})
        facets = Facets(
            source=[FacetBucket(**b) for b in facets_data.get("source", [])],
            author=[FacetBucket(**b) for b in facets_data.get("author", [])],
            tags=[FacetBucket(**b) for b in facets_data.get("tags", [])],
        )
        hits = []
        for h in data.get("hits", []):
            doc_data = h.get("doc", {})
            doc = Document(**doc_data)
            hits.append(SearchHit(doc=doc, score=h.get("score", 0.0), snippet=h.get("snippet")))

        return SearchResponse(
            total=data.get("total", 0),
            hits=hits,
            facets=facets,
            query=data.get("query", ""),
            elapsed_ms=data.get("elapsed_ms", 0),
        )
