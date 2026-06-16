use tantivy::query::Query;
use tantivy::schema::Schema;
use tantivy::snippet::SnippetGenerator;
use tantivy::Searcher;
use tantivy::TantivyDocument;

pub fn generate_snippets(
    searcher: &Searcher,
    query: &Box<dyn Query>,
    tantivy_doc: &TantivyDocument,
    _schema: &Schema,
) -> Option<String> {
    let title_field = _schema.get_field("title").ok()?;
    let content_field = _schema.get_field("content").ok()?;

    let title_snippet = generate_field_snippet(searcher, query, title_field, tantivy_doc);
    let content_snippet = generate_field_snippet(searcher, query, content_field, tantivy_doc);

    match (title_snippet, content_snippet) {
        (Some(t), Some(c)) if has_highlight(&t) => Some(format!("{}\n{}", t, c)),
        (Some(_), Some(c)) if has_highlight(&c) => Some(c),
        (Some(c), _) => Some(c),
        _ => None,
    }
}

fn generate_field_snippet(
    searcher: &Searcher,
    query: &Box<dyn Query>,
    field: tantivy::schema::Field,
    tantivy_doc: &TantivyDocument,
) -> Option<String> {
    let generator = SnippetGenerator::create(searcher, query.as_ref(), field).ok()?;

    let snippet = generator.snippet_from_doc(tantivy_doc);

    let html = snippet.to_html();
    if html.is_empty() {
        None
    } else {
        Some(html)
    }
}

fn has_highlight(html: &str) -> bool {
    html.contains("<em>")
}
