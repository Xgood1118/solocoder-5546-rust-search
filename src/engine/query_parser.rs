use std::ops::Bound;

use tantivy::query::*;
use tantivy::schema::{Field, IndexRecordOption, Schema};

use crate::api::search::SearchFilters;

pub struct SearchQueryParser<'a> {
    schema: &'a Schema,
}

impl<'a> SearchQueryParser<'a> {
    pub fn new(schema: &'a Schema) -> Self {
        Self { schema }
    }

    pub fn parse(
        &self,
        query_str: &str,
        filters: &SearchFilters,
    ) -> Result<Box<dyn Query>, String> {
        let mut queries: Vec<(Occur, Box<dyn Query>)> = Vec::new();

        let main_query = self.parse_query_string(query_str)?;
        queries.push((Occur::Must, main_query));

        let filter_queries = self.build_filter_queries(filters)?;
        for fq in filter_queries {
            queries.push((Occur::Must, fq));
        }

        if queries.len() == 1 {
            Ok(queries.into_iter().next().unwrap().1)
        } else {
            Ok(Box::new(BooleanQuery::new(queries)))
        }
    }

    fn parse_query_string(&self, query_str: &str) -> Result<Box<dyn Query>, String> {
        let query_str = query_str.trim();
        if query_str.is_empty() {
            return Ok(Box::new(AllQuery));
        }

        let tokens = self.tokenize_query(query_str);
        self.build_query_from_tokens(&tokens)
    }

    fn tokenize_query(&self, input: &str) -> Vec<QueryToken> {
        let mut tokens = Vec::new();
        let mut chars = input.chars().peekable();

        while let Some(&ch) = chars.peek() {
            if ch == '"' {
                chars.next();
                let mut phrase = String::new();
                while let Some(&pc) = chars.peek() {
                    if pc == '"' {
                        chars.next();
                                break;
                    }
                    phrase.push(chars.next().unwrap());
                    }
                if !phrase.is_empty() {
                    tokens.push(QueryToken::Phrase(phrase));
                }
            } else if ch == '(' {
                chars.next();
                tokens.push(QueryToken::LParen);
            } else if ch == ')' {
                chars.next();
                tokens.push(QueryToken::RParen);
            } else if ch == '~' {
                chars.next();
                let mut dist_str = String::new();
                while let Some(&dc) = chars.peek() {
                    if dc.is_ascii_digit() {
                        dist_str.push(chars.next().unwrap());
                            } else {
                        break;
                    }
                }
                let distance = dist_str.parse::<u8>().unwrap_or(2);
                if let Some(last) = tokens.last_mut() {
                    if let QueryToken::Term(ref t) = last {
                        let term = t.clone();
                        *last = QueryToken::Fuzzy(term, distance);
                    }
                }
            } else if ch.is_whitespace() {
                chars.next();
            } else {
                let mut word = String::new();
                while let Some(&wc) = chars.peek() {
                    if wc.is_whitespace() || wc == '"' || wc == '(' || wc == ')' || wc == '~' {
                        break;
                    }
                    word.push(chars.next().unwrap());
                    }

                let upper = word.to_uppercase();
                if upper == "AND" {
                    tokens.push(QueryToken::And);
                } else if upper == "OR" {
                    tokens.push(QueryToken::Or);
                } else if upper == "NOT" {
                    tokens.push(QueryToken::Not);
                } else if word.contains(':') {
                    let parts: Vec<&str> = word.splitn(2, ':').collect();
                    if parts.len() == 2 && !parts[1].is_empty() {
                        tokens.push(QueryToken::Field(parts[0].to_string(), parts[1].to_string()));
                    } else {
                        tokens.push(QueryToken::Term(word));
                    }
                } else {
                    tokens.push(QueryToken::Term(word));
                }
            }
        }

        tokens
    }

    fn build_query_from_tokens(&self, tokens: &[QueryToken]) -> Result<Box<dyn Query>, String> {
        if tokens.is_empty() {
            return Ok(Box::new(AllQuery));
        }

        let mut result = Vec::new();
        let mut i = 0;
        let mut default_occur = Occur::Should;

        while i < tokens.len() {
            match &tokens[i] {
                QueryToken::And => {
                    default_occur = Occur::Must;
                    i += 1;
                }
                QueryToken::Or => {
                    default_occur = Occur::Should;
                    i += 1;
                }
                QueryToken::Not => {
                    if i + 1 < tokens.len() {
                        let neg_query = self.token_to_query(&tokens[i + 1])?;
                        result.push((Occur::MustNot, neg_query));
                        i += 2;
                    } else {
                        i += 1;
                    }
                }
                _ => {
                    let query = self.token_to_query(&tokens[i])?;
                    result.push((default_occur, query));
                    default_occur = Occur::Should;
                    i += 1;
                }
            }
        }

        if result.is_empty() {
            return Ok(Box::new(AllQuery));
        }

        if result.len() == 1 {
            let (occur, query) = result.into_iter().next().unwrap();
            match occur {
                Occur::MustNot => Ok(Box::new(BooleanQuery::new(vec![
                    (Occur::Must, Box::new(AllQuery)),
                    (Occur::MustNot, query),
                ]))),
                _ => Ok(query),
            }
        } else {
            Ok(Box::new(BooleanQuery::new(result)))
        }
    }

    fn token_to_query(&self, token: &QueryToken) -> Result<Box<dyn Query>, String> {
        match token {
            QueryToken::Term(term) => {
                Ok(self.build_multi_field_term_query(term))
            }
            QueryToken::Phrase(phrase) => {
                Ok(self.build_multi_field_phrase_query(phrase))
            }
            QueryToken::Fuzzy(term, distance) => {
                Ok(self.build_fuzzy_query(term, *distance))
            }
            QueryToken::Field(field_name, value) => {
                self.build_field_query(field_name, value)
            }
            _ => Ok(Box::new(AllQuery)),
        }
    }

    fn build_multi_field_term_query(&self, term: &str) -> Box<dyn Query> {
        let fields = self.get_searchable_fields();
        let mut queries: Vec<(Occur, Box<dyn Query>)> = Vec::new();

        for field in &fields {
            let field_name = self.schema.get_field_name(*field);
            let boost = self.get_field_boost(field_name);

            let term_obj = tantivy::Term::from_field_text(*field, term);
            let tq: Box<dyn Query> = Box::new(TermQuery::new(term_obj, IndexRecordOption::WithFreqsAndPositions));

            if boost != 1.0 {
                queries.push((Occur::Should, Box::new(BoostQuery::new(tq, boost))));
            } else {
                queries.push((Occur::Should, tq));
            }
        }

        if queries.is_empty() {
            Box::new(EmptyQuery)
        } else if queries.len() == 1 {
            queries.into_iter().next().unwrap().1
        } else {
            Box::new(BooleanQuery::new(queries))
        }
    }

    fn build_multi_field_phrase_query(&self, phrase: &str) -> Box<dyn Query> {
        let fields = self.get_searchable_fields();
        let mut queries: Vec<(Occur, Box<dyn Query>)> = Vec::new();

        for field in &fields {
            let field_name = self.schema.get_field_name(*field);
            let boost = self.get_field_boost(field_name);

            let jieba = jieba_rs::Jieba::new();
            let tokens = jieba.tokenize(phrase, jieba_rs::TokenizeMode::Search, false);

            let terms: Vec<(usize, tantivy::Term)> = tokens
                .iter()
                .enumerate()
                .map(|(pos, t)| {
                    (pos, tantivy::Term::from_field_text(*field, &t.word))
                })
                .collect();

            if terms.is_empty() {
                continue;
            }

            let pq: Box<dyn Query> = Box::new(PhraseQuery::new_with_offset(terms));

            if boost != 1.0 {
                queries.push((Occur::Should, Box::new(BoostQuery::new(pq, boost))));
            } else {
                queries.push((Occur::Should, pq));
            }
        }

        if queries.is_empty() {
            Box::new(EmptyQuery)
        } else if queries.len() == 1 {
            queries.into_iter().next().unwrap().1
        } else {
            Box::new(BooleanQuery::new(queries))
        }
    }

    fn build_fuzzy_query(&self, term: &str, distance: u8) -> Box<dyn Query> {
        let fields = self.get_searchable_fields();
        let mut queries: Vec<(Occur, Box<dyn Query>)> = Vec::new();

        for field in &fields {
            let fq: Box<dyn Query> = Box::new(FuzzyTermQuery::new_prefix(
                tantivy::Term::from_field_text(*field, term),
                distance,
                true,
            ));
            queries.push((Occur::Should, fq));
        }

        if queries.is_empty() {
            Box::new(EmptyQuery)
        } else if queries.len() == 1 {
            queries.into_iter().next().unwrap().1
        } else {
            Box::new(BooleanQuery::new(queries))
        }
    }

    fn build_field_query(&self, field_name: &str, value: &str) -> Result<Box<dyn Query>, String> {
        let field = self.schema.get_field(field_name).map_err(|_| {
            format!("Unknown field: {}", field_name)
        })?;

        if value.starts_with('"') && value.ends_with('"') && value.len() >= 2 {
            let phrase = &value[1..value.len() - 1];
            let jieba = jieba_rs::Jieba::new();
            let tokens = jieba.tokenize(phrase, jieba_rs::TokenizeMode::Search, false);
            let terms: Vec<(usize, tantivy::Term)> = tokens
                .iter()
                .enumerate()
                .map(|(pos, t)| (pos, tantivy::Term::from_field_text(field, &t.word)))
                .collect();
            if terms.is_empty() {
                return Ok(Box::new(EmptyQuery));
            }
            Ok(Box::new(PhraseQuery::new_with_offset(terms)))
        } else if value.ends_with('~') {
            let term_str = value.trim_end_matches('~');
            Ok(Box::new(FuzzyTermQuery::new_prefix(
                tantivy::Term::from_field_text(field, term_str),
                2,
                true,
            )))
        } else {
            Ok(Box::new(TermQuery::new(
                tantivy::Term::from_field_text(field, value),
                IndexRecordOption::WithFreqsAndPositions,
            )))
        }
    }

    fn get_searchable_fields(&self) -> Vec<Field> {
        let mut fields = Vec::new();
        for field in self.schema.fields() {
            let field_name = self.schema.get_field_name(field.0);
            if matches!(field_name, "title" | "content" | "tags" | "author") {
                fields.push(field.0);
            }
        }
        fields
    }

    fn get_field_boost(&self, field_name: &str) -> f32 {
        match field_name {
            "title" => 3.0,
            "tags" => 2.0,
            _ => 1.0,
        }
    }

    fn build_filter_queries(&self, filters: &SearchFilters) -> Result<Vec<Box<dyn Query>>, String> {
        let mut queries: Vec<Box<dyn Query>> = Vec::new();

        if let Some(ref source) = filters.source {
            if let Ok(field) = self.schema.get_field("source") {
                let mut sub_queries: Vec<(Occur, Box<dyn Query>)> = Vec::new();
                for s in source {
                    sub_queries.push((
                        Occur::Should,
                        Box::new(TermQuery::new(
                            tantivy::Term::from_field_text(field, s),
                            IndexRecordOption::Basic,
                        )),
                    ));
                }
                queries.push(Box::new(BooleanQuery::new(sub_queries)) as Box<dyn Query>);
            }
        }

        if let Some(ref author) = filters.author {
            if let Ok(field) = self.schema.get_field("author") {
                let mut sub_queries: Vec<(Occur, Box<dyn Query>)> = Vec::new();
                for a in author {
                    sub_queries.push((
                        Occur::Should,
                        Box::new(TermQuery::new(
                            tantivy::Term::from_field_text(field, a),
                            IndexRecordOption::Basic,
                        )),
                    ));
                }
                queries.push(Box::new(BooleanQuery::new(sub_queries)) as Box<dyn Query>);
            }
        }

        if let Some(ref tags) = filters.tags {
            if let Ok(field) = self.schema.get_field("tags") {
                let mut sub_queries: Vec<(Occur, Box<dyn Query>)> = Vec::new();
                for t in tags {
                    sub_queries.push((
                        Occur::Should,
                        Box::new(TermQuery::new(
                            tantivy::Term::from_field_text(field, t),
                            IndexRecordOption::Basic,
                        )),
                    ));
                }
                queries.push(Box::new(BooleanQuery::new(sub_queries)) as Box<dyn Query>);
            }
        }

        if let (Some(from), Some(to)) = (filters.from_date, filters.to_date) {
            let field_name = "updated_at".to_string();
            let range: Box<dyn Query> = Box::new(
                tantivy::query::RangeQuery::new_i64_bounds(
                    field_name,
                    Bound::Included(from),
                    Bound::Included(to),
                ),
            );
            queries.push(range);
        } else if let Some(from) = filters.from_date {
            let field_name = "updated_at".to_string();
            let range: Box<dyn Query> = Box::new(
                tantivy::query::RangeQuery::new_i64_bounds(
                    field_name,
                    Bound::Included(from),
                    Bound::Unbounded,
                ),
            );
            queries.push(range);
        } else if let Some(to) = filters.to_date {
            let field_name = "updated_at".to_string();
            let range: Box<dyn Query> = Box::new(
                tantivy::query::RangeQuery::new_i64_bounds(
                    field_name,
                    Bound::Unbounded,
                    Bound::Included(to),
                ),
            );
            queries.push(range);
        }

        Ok(queries)
    }
}

#[derive(Debug, Clone)]
enum QueryToken {
    Term(String),
    Phrase(String),
    Fuzzy(String, u8),
    Field(String, String),
    And,
    Or,
    Not,
    LParen,
    RParen,
}
