//! Keyword search helpers for the SQLite FTS5 fallback.
//!
//! The raw query used to go straight into `memories_fts MATCH ?`, which treats
//! the text as FTS5 syntax: `north-dock` became "north NOT column dock" and
//! failed with `no such column`, and a natural question only matched memories
//! containing every word in it. These helpers turn free text into a safe
//! query that matches on any meaningful word, and score hits by how many of
//! those words they contain.

use std::collections::HashSet;

/// Words too common to say anything about relevance.
const STOPWORDS: &[&str] = &[
    "a", "about", "after", "all", "also", "am", "an", "and", "any", "are", "as", "at", "be",
    "been", "before", "but", "by", "can", "could", "did", "do", "does", "for", "from", "had",
    "has", "have", "he", "her", "him", "his", "how", "i", "if", "in", "into", "is", "it", "its",
    "me", "my", "no", "not", "of", "on", "or", "our", "she", "should", "so", "than", "that",
    "the", "their", "them", "then", "there", "these", "they", "this", "to", "us", "was", "we",
    "were", "what", "when", "where", "which", "who", "why", "will", "with", "would", "you",
    "your",
];

/// Upper bound on terms in one query, to keep the FTS expression small.
const MAX_TERMS: usize = 16;

/// Splits free text into lowercase search terms, the same way FTS5's default
/// `unicode61` tokenizer does (runs of letters and digits), dropping stopwords
/// and duplicates. If every word is a stopword, the stopwords are kept so a
/// query like "the" still searches for something.
pub fn terms(query: &str) -> Vec<String> {
    let words: Vec<String> = query
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_lowercase)
        .collect();

    let meaningful = dedupe(
        words
            .iter()
            .filter(|word| word.chars().count() >= 2 && !STOPWORDS.contains(&word.as_str())),
    );
    if !meaningful.is_empty() {
        return meaningful;
    }
    dedupe(words.iter())
}

fn dedupe<'a>(words: impl Iterator<Item = &'a String>) -> Vec<String> {
    let mut seen = HashSet::new();
    words
        .filter(|word| seen.insert(word.as_str()))
        .take(MAX_TERMS)
        .cloned()
        .collect()
}

/// Builds an FTS5 MATCH expression that matches any of `terms`. Each term is
/// a run of letters and digits, so wrapping it in double quotes makes it a
/// plain string token that FTS5 can't read as an operator or column filter.
/// Returns `None` when there is nothing to search for.
pub fn match_expression(terms: &[String]) -> Option<String> {
    if terms.is_empty() {
        return None;
    }
    Some(
        terms
            .iter()
            .map(|term| format!("\"{term}\""))
            .collect::<Vec<_>>()
            .join(" OR "),
    )
}

/// Fraction of `terms` that appear as words in `content`, in `0.0..=1.0`.
pub fn score(content: &str, terms: &[String]) -> f32 {
    if terms.is_empty() {
        return 0.0;
    }
    let words: HashSet<String> = content
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_lowercase)
        .collect();
    let matched = terms.iter().filter(|term| words.contains(*term)).count();
    matched as f32 / terms.len() as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hyphens_and_punctuation_become_plain_terms() {
        assert_eq!(terms("north-dock"), vec!["north", "dock"]);
        assert_eq!(terms("e-mail: \"prefs\" (v2)"), vec!["mail", "prefs", "v2"]);
        assert_eq!(
            match_expression(&terms("north-dock")).as_deref(),
            Some("\"north\" OR \"dock\"")
        );
    }

    #[test]
    fn questions_drop_stopwords() {
        assert_eq!(
            terms("How should we contact them?"),
            vec!["contact"]
        );
        assert_eq!(
            terms("which dock does the robot prefer"),
            vec!["dock", "robot", "prefer"]
        );
    }

    #[test]
    fn all_stopwords_still_search() {
        assert_eq!(terms("the"), vec!["the"]);
    }

    #[test]
    fn empty_and_symbol_only_queries_have_no_expression() {
        assert!(match_expression(&terms("")).is_none());
        assert!(match_expression(&terms("-- !! ??")).is_none());
    }

    #[test]
    fn duplicates_are_removed_and_terms_capped() {
        assert_eq!(terms("Dock dock DOCK"), vec!["dock"]);
        let long = (0..40).map(|i| format!("word{i}")).collect::<Vec<_>>().join(" ");
        assert_eq!(terms(&long).len(), MAX_TERMS);
    }

    #[test]
    fn uuids_split_into_hex_runs() {
        let t = terms("97f7ebb5-1cd0-4332-ab63-3baa80e6a7fd");
        assert_eq!(t.first().map(String::as_str), Some("97f7ebb5"));
        assert!(match_expression(&t).unwrap().contains("\"3baa80e6a7fd\""));
    }

    #[test]
    fn score_counts_matched_terms() {
        let t = terms("which dock does the robot prefer");
        assert_eq!(score("The robot prefers the north dock", &t), 2.0 / 3.0);
        assert_eq!(score("Robot prefer dock", &t), 1.0);
        assert_eq!(score("nothing relevant", &t), 0.0);
    }
}
