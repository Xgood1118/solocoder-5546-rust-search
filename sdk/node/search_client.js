/**
 * Unified Search Node.js SDK
 */
class FacetBucket {
  constructor(value, count) {
    this.value = value;
    this.count = count;
  }
}

class Facets {
  constructor(source = [], author = [], tags = []) {
    this.source = source;
    this.author = author;
    this.tags = tags;
  }
}

class Document {
  constructor({ source, source_id, title, content, url, author, created_at, updated_at, tags, project }) {
    this.source = source;
    this.source_id = source_id;
    this.title = title;
    this.content = content;
    this.url = url || null;
    this.author = author || null;
    this.created_at = created_at;
    this.updated_at = updated_at;
    this.tags = tags || [];
    this.project = project || null;
  }
}

class SearchHit {
  constructor(doc, score, snippet) {
    this.doc = doc;
    this.score = score;
    this.snippet = snippet || null;
  }
}

class SearchResponse {
  constructor(total, hits, facets, query, elapsed_ms) {
    this.total = total;
    this.hits = hits;
    this.facets = facets;
    this.query = query;
    this.elapsed_ms = elapsed_ms;
  }
}

class UnifiedSearchClient {
  constructor(baseUrl = "http://localhost:8340") {
    this.baseUrl = baseUrl.replace(/\/+$/, "");
  }

  async search(q, { limit = 20, offset = 0, source, author, tags, from_date, to_date } = {}) {
    const body = { q, limit, offset };
    if (source) body.source = source;
    if (author) body.author = author;
    if (tags) body.tags = tags;
    if (from_date) body.from_date = from_date;
    if (to_date) body.to_date = to_date;

    const resp = await fetch(`${this.baseUrl}/search`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(body),
    });
    if (!resp.ok) throw new Error(`Search failed: ${resp.status} ${await resp.text()}`);
    const data = await resp.json();
    return this._parseResponse(data);
  }

  async indexDocument(doc) {
    const payload = {
      source: doc.source,
      source_id: doc.source_id,
      title: doc.title,
      content: doc.content,
      created_at: doc.created_at || "2025-01-01T00:00:00Z",
      updated_at: doc.updated_at || "2025-01-01T00:00:00Z",
      tags: doc.tags || [],
    };
    if (doc.url) payload.url = doc.url;
    if (doc.author) payload.author = doc.author;
    if (doc.project) payload.project = doc.project;

    const resp = await fetch(`${this.baseUrl}/documents`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(payload),
    });
    if (!resp.ok) throw new Error(`Index failed: ${resp.status} ${await resp.text()}`);
    return resp.json();
  }

  async forceCommit() {
    const resp = await fetch(`${this.baseUrl}/commit`, { method: "POST" });
    if (!resp.ok) throw new Error(`Commit failed: ${resp.status} ${await resp.text()}`);
    return resp.json();
  }

  async fetchConnector(sourceType, params, lastFetchedAt = null) {
    const body = { source_type: sourceType, params };
    if (lastFetchedAt) body.last_fetched_at = lastFetchedAt;

    const resp = await fetch(`${this.baseUrl}/connectors/fetch`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(body),
    });
    if (!resp.ok) throw new Error(`Fetch failed: ${resp.status} ${await resp.text()}`);
    return resp.json();
  }

  async listConnectors() {
    const resp = await fetch(`${this.baseUrl}/connectors`);
    if (!resp.ok) throw new Error(`List failed: ${resp.status} ${await resp.text()}`);
    return resp.json();
  }

  _parseResponse(data) {
    const facetsData = data.facets || {};
    const facets = new Facets(
      (facetsData.source || []).map((b) => new FacetBucket(b.value, b.count)),
      (facetsData.author || []).map((b) => new FacetBucket(b.value, b.count)),
      (facetsData.tags || []).map((b) => new FacetBucket(b.value, b.count)),
    );
    const hits = (data.hits || []).map(
      (h) => new SearchHit(new Document(h.doc || {}), h.score || 0, h.snippet),
    );
    return new SearchResponse(data.total || 0, hits, facets, data.query || "", data.elapsed_ms || 0);
  }
}

module.exports = { UnifiedSearchClient, Document, SearchHit, SearchResponse, Facets, FacetBucket };
