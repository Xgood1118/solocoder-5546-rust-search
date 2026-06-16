use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use tantivy::collector::TopDocs;
use tantivy::query::{BooleanQuery, Occur, Query, TermQuery};
use tantivy::schema::*;
use tantivy::{doc, Index, IndexWriter, ReloadPolicy};
use tokio::sync::RwLock;

use crate::engine::analyzer::register_custom_analyzers;
use crate::engine::highlight::generate_snippets;
use crate::engine::query_parser::SearchQueryParser;
use crate::schema::document::{Document, FacetBucket, Facets, SearchHit, SearchResponse};

pub struct IndexManager {
    index: Index,
    writer: Arc<RwLock<IndexWriter>>,
    schema: Schema,
    dirty: Arc<RwLock<bool>>,
}

impl IndexManager {
    pub fn field_source(&self) -> Field {
        self.schema.get_field("source").unwrap()
    }
    pub fn field_source_id(&self) -> Field {
        self.schema.get_field("source_id").unwrap()
    }
    pub fn field_title(&self) -> Field {
        self.schema.get_field("title").unwrap()
    }
    pub fn field_content(&self) -> Field {
        self.schema.get_field("content").unwrap()
    }
    pub fn field_url(&self) -> Field {
        self.schema.get_field("url").unwrap()
    }
    pub fn field_author(&self) -> Field {
        self.schema.get_field("author").unwrap()
    }
    pub fn field_created_at(&self) -> Field {
        self.schema.get_field("created_at").unwrap()
    }
    pub fn field_updated_at(&self) -> Field {
        self.schema.get_field("updated_at").unwrap()
    }
    pub fn field_tags(&self) -> Field {
        self.schema.get_field("tags").unwrap()
    }
    pub fn field_project(&self) -> Field {
        self.schema.get_field("project").unwrap()
    }

    pub fn build_schema() -> Schema {
        let mut builder = Schema::builder();

        builder.add_text_field("source", STRING | STORED);
        builder.add_text_field("source_id", STRING | STORED);
        builder.add_text_field(
            "title",
            TextOptions::default()
                .set_indexing_options(
                    TextFieldIndexing::default()
                        .set_tokenizer("default_mixed")
                        .set_index_option(IndexRecordOption::WithFreqsAndPositions),
                )
                .set_stored(),
        );
        builder.add_text_field(
            "content",
            TextOptions::default()
                .set_indexing_options(
                    TextFieldIndexing::default()
                        .set_tokenizer("default_mixed")
                        .set_index_option(IndexRecordOption::WithFreqsAndPositions),
                )
                .set_stored(),
        );
        builder.add_text_field("url", STRING | STORED);
        builder.add_text_field(
            "author",
            TextOptions::default()
                .set_indexing_options(
                    TextFieldIndexing::default()
                        .set_tokenizer("default_mixed")
                        .set_index_option(IndexRecordOption::WithFreqsAndPositions),
                )
                .set_stored(),
        );
        builder.add_i64_field("created_at", STORED | INDEXED);
        builder.add_i64_field("updated_at", STORED | INDEXED);
        builder.add_text_field(
            "tags",
            TextOptions::default()
                .set_indexing_options(
                    TextFieldIndexing::default()
                        .set_tokenizer("default_mixed")
                        .set_index_option(IndexRecordOption::WithFreqsAndPositions),
                )
                .set_stored(),
        );
        builder.add_text_field("project", STRING | STORED);

        builder.build()
    }

    pub fn open(index_dir: &str) -> Result<Self, String> {
        let schema = Self::build_schema();
        let path = Path::new(index_dir);

        let index = if path.exists() && path.read_dir().map_or(false, |mut d| d.next().is_some()) {
            Index::open_in_dir(path).map_err(|e| format!("Failed to open index: {}", e))?
        } else {
            std::fs::create_dir_all(path).map_err(|e| format!("Failed to create index dir: {}", e))?;
            Index::create_in_dir(path, schema.clone()).map_err(|e| format!("Failed to create index: {}", e))?
        };

        register_custom_analyzers(&index);

        let writer = index
            .writer(50_000_000)
            .map_err(|e| format!("Failed to create index writer: {}", e))?;

        Ok(Self {
            index,
            writer: Arc::new(RwLock::new(writer)),
            schema,
            dirty: Arc::new(RwLock::new(false)),
        })
    }

    pub async fn add_document(&self, doc: &Document) -> Result<(), String> {
        let writer = self.writer.write().await;

        let source_term = tantivy::Term::from_field_text(self.field_source(), &doc.source);
        let source_id_term = tantivy::Term::from_field_text(self.field_source_id(), &doc.source_id);

        let delete_query: Box<dyn Query> = Box::new(BooleanQuery::new(vec![
            (
                Occur::Must,
                Box::new(TermQuery::new(source_term, IndexRecordOption::Basic)) as Box<dyn Query>,
            ),
            (
                Occur::Must,
                Box::new(TermQuery::new(source_id_term, IndexRecordOption::Basic)) as Box<dyn Query>,
            ),
        ]));

        writer.delete_query(delete_query).map_err(|e| format!("Delete failed: {}", e))?;

        let mut tantivy_doc = doc!(
            self.field_source() => doc.source.as_str(),
            self.field_source_id() => doc.source_id.as_str(),
            self.field_title() => doc.title.as_str(),
            self.field_content() => doc.content.as_str(),
            self.field_url() => doc.url.as_deref().unwrap_or(""),
            self.field_author() => doc.author.as_deref().unwrap_or(""),
            self.field_created_at() => doc.created_at.timestamp(),
            self.field_updated_at() => doc.updated_at.timestamp(),
        );

        for tag in &doc.tags {
            tantivy_doc.add_text(self.field_tags(), tag);
        }
        if let Some(ref project) = doc.project {
            tantivy_doc.add_text(self.field_project(), project);
        }

        writer.add_document(tantivy_doc).map_err(|e| format!("Add doc failed: {}", e))?;

        drop(writer);
        *self.dirty.write().await = true;

        Ok(())
    }

    pub async fn add_documents(&self, docs: &[Document]) -> Result<usize, String> {
        let writer = self.writer.write().await;
        let mut count = 0;

        for doc in docs {
            let source_term = tantivy::Term::from_field_text(self.field_source(), &doc.source);
            let source_id_term = tantivy::Term::from_field_text(self.field_source_id(), &doc.source_id);

            let delete_query: Box<dyn Query> = Box::new(BooleanQuery::new(vec![
                (
                    Occur::Must,
                    Box::new(TermQuery::new(source_term, IndexRecordOption::Basic)) as Box<dyn Query>,
                ),
                (
                    Occur::Must,
                    Box::new(TermQuery::new(source_id_term, IndexRecordOption::Basic)) as Box<dyn Query>,
                ),
            ]));

            writer.delete_query(delete_query).map_err(|e| format!("Delete failed: {}", e))?;

            let mut tantivy_doc = doc!(
                self.field_source() => doc.source.as_str(),
                self.field_source_id() => doc.source_id.as_str(),
                self.field_title() => doc.title.as_str(),
                self.field_content() => doc.content.as_str(),
                self.field_url() => doc.url.as_deref().unwrap_or(""),
                self.field_author() => doc.author.as_deref().unwrap_or(""),
                self.field_created_at() => doc.created_at.timestamp(),
                self.field_updated_at() => doc.updated_at.timestamp(),
            );

            for tag in &doc.tags {
                tantivy_doc.add_text(self.field_tags(), tag);
            }
            if let Some(ref project) = doc.project {
                tantivy_doc.add_text(self.field_project(), project);
            }

            writer.add_document(tantivy_doc).map_err(|e| format!("Add doc failed: {}", e))?;
            count += 1;
        }

        drop(writer);
        *self.dirty.write().await = true;

        Ok(count)
    }

    pub async fn commit(&self) -> Result<(), String> {
        let mut writer = self.writer.write().await;
        writer.commit().map_err(|e| format!("Commit failed: {}", e))?;
        drop(writer);
        *self.dirty.write().await = false;
        Ok(())
    }

    pub async fn force_commit(&self) -> Result<(), String> {
        self.commit().await
    }

    pub async fn commit_if_dirty(&self) -> Result<(), String> {
        if *self.dirty.read().await {
            self.commit().await
        } else {
            Ok(())
        }
    }

    pub fn search(
        &self,
        query_str: &str,
        filters: &crate::api::search::SearchFilters,
        limit: usize,
        offset: usize,
    ) -> Result<SearchResponse, String> {
        let start = std::time::Instant::now();

        let reader = self
            .index
            .reader_builder()
            .reload_policy(ReloadPolicy::Manual)
            .try_into()
            .map_err(|e| format!("Reader error: {}", e))?;

        reader.reload().map_err(|e| format!("Reload error: {}", e))?;

        let searcher = reader.searcher();

        let parser = SearchQueryParser::new(&self.schema);
        let query = parser.parse(query_str, filters)?;

        let top_docs_collector = TopDocs::with_limit(limit + offset);

        let top_docs = searcher
            .search(&query, &top_docs_collector)
            .map_err(|e| format!("Search error: {}", e))?;

        let total = top_docs.len();
        let page: Vec<(f32, tantivy::DocAddress)> = top_docs
            .into_iter()
            .skip(offset)
            .take(limit)
            .collect();

        let mut hits = Vec::new();
        let mut source_counts: HashMap<String, usize> = HashMap::new();
        let mut author_counts: HashMap<String, usize> = HashMap::new();
        let mut tag_counts: HashMap<String, usize> = HashMap::new();

        for (score, doc_address) in &page {
            let tantivy_doc: TantivyDocument = searcher
                .doc(*doc_address)
                .map_err(|e| format!("Doc fetch error: {}", e))?;

            let document = self.tantivy_doc_to_domain(&tantivy_doc)?;
            let snippet = generate_snippets(&searcher, &query, &tantivy_doc, &self.schema);

            *source_counts.entry(document.source.clone()).or_insert(0) += 1;
            if let Some(ref author) = document.author {
                if !author.is_empty() {
                    *author_counts.entry(author.clone()).or_insert(0) += 1;
                }
            }
            for tag in &document.tags {
                *tag_counts.entry(tag.clone()).or_insert(0) += 1;
            }

            hits.push(SearchHit {
                doc: document,
                score: *score,
                snippet,
            });
        }

        let facets = Facets {
            source: to_buckets(source_counts),
            author: to_buckets(author_counts),
            tags: to_buckets(tag_counts),
        };

        let elapsed_ms = start.elapsed().as_millis() as u64;

        Ok(SearchResponse {
            total,
            hits,
            facets,
            query: query_str.to_string(),
            elapsed_ms,
        })
    }

    fn tantivy_doc_to_domain(&self, doc: &TantivyDocument) -> Result<Document, String> {
        let source = doc
            .get_first(self.field_source())
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let source_id = doc
            .get_first(self.field_source_id())
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let title = doc
            .get_first(self.field_title())
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let content = doc
            .get_first(self.field_content())
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let url = doc
            .get_first(self.field_url())
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let author = doc
            .get_first(self.field_author())
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let created_at_ts = doc
            .get_first(self.field_created_at())
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        let updated_at_ts = doc
            .get_first(self.field_updated_at())
            .and_then(|v| v.as_i64())
            .unwrap_or(0);

        let created_at = chrono::DateTime::from_timestamp(created_at_ts, 0)
            .unwrap_or_else(|| chrono::DateTime::from_timestamp(0, 0).unwrap());
        let updated_at = chrono::DateTime::from_timestamp(updated_at_ts, 0)
            .unwrap_or_else(|| chrono::DateTime::from_timestamp(0, 0).unwrap());

        let tags: Vec<String> = doc
            .get_all(self.field_tags())
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect();

        let project = doc
            .get_first(self.field_project())
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        Ok(Document {
            source,
            source_id,
            title,
            content,
            url,
            author,
            created_at,
            updated_at,
            tags,
            project,
        })
    }

    pub fn index(&self) -> &Index {
        &self.index
    }
}

fn to_buckets(counts: HashMap<String, usize>) -> Vec<FacetBucket> {
    let mut buckets: Vec<FacetBucket> = counts
        .into_iter()
        .map(|(value, count)| FacetBucket { value, count })
        .collect();
    buckets.sort_by(|a, b| b.count.cmp(&a.count));
    buckets
}
