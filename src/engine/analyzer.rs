use jieba_rs::Jieba;
use tantivy::tokenizer::{LowerCaser, SimpleTokenizer, TextAnalyzer, Token, TokenStream, Tokenizer};

static JIEBA_INSTANCE: std::sync::OnceLock<Jieba> = std::sync::OnceLock::new();

fn get_jieba() -> &'static Jieba {
    JIEBA_INSTANCE.get_or_init(Jieba::new)
}

pub fn chinese_analyzer() -> TextAnalyzer {
    TextAnalyzer::builder(JiebaTokenizer::default())
        .filter(LowerCaser)
        .build()
}

pub fn english_analyzer() -> TextAnalyzer {
    TextAnalyzer::builder(SimpleTokenizer::default())
        .filter(LowerCaser)
        .build()
}

pub fn default_analyzer() -> TextAnalyzer {
    TextAnalyzer::builder(JiebaTokenizer::default())
        .filter(LowerCaser)
        .build()
}

#[derive(Clone, Default)]
pub struct JiebaTokenizer;

impl Tokenizer for JiebaTokenizer {
    type TokenStream<'a> = JiebaTokenStream;

    fn token_stream<'a>(&'a mut self, text: &'a str) -> Self::TokenStream<'a> {
        let jieba = get_jieba();
        let tokens = jieba.tokenize(text, jieba_rs::TokenizeMode::Search, false);
        let tokens: Vec<Token> = tokens
            .iter()
            .map(|t| Token {
                offset_from: t.start,
                offset_to: t.end,
                text: t.word.to_string(),
                position: t.start,
                position_length: t.end - t.start,
            })
            .collect();
        JiebaTokenStream {
            tokens,
            index: 0,
            current: Token::default(),
        }
    }
}

pub struct JiebaTokenStream {
    tokens: Vec<Token>,
    index: usize,
    current: Token,
}

impl TokenStream for JiebaTokenStream {
    fn advance(&mut self) -> bool {
        if self.index < self.tokens.len() {
            self.current = self.tokens[self.index].clone();
            self.index += 1;
            true
        } else {
            false
        }
    }

    fn token(&self) -> &Token {
        &self.current
    }

    fn token_mut(&mut self) -> &mut Token {
        &mut self.current
    }
}

pub const CHINESE_ANALYZER_NAME: &str = "chinese_jieba";
pub const ENGLISH_ANALYZER_NAME: &str = "english_standard";
pub const DEFAULT_ANALYZER_NAME: &str = "default_mixed";

pub fn register_custom_analyzers(index: &tantivy::Index) {
    index
        .tokenizers()
        .register(CHINESE_ANALYZER_NAME, chinese_analyzer());
    index
        .tokenizers()
        .register(ENGLISH_ANALYZER_NAME, english_analyzer());
    index
        .tokenizers()
        .register(DEFAULT_ANALYZER_NAME, default_analyzer());
}
