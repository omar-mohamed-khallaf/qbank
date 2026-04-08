use regex::Regex;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedQuestion {
    pub question_number: i32,
    pub question_text: String,
}

static QUESTION_ANCHOR_REGEX: std::sync::LazyLock<Regex> =
    std::sync::LazyLock::new(|| Regex::new(r"(\d+)\s*[\)\-\.]\s*").expect("valid regex"));

static QUESTION_LINE_REGEX: std::sync::LazyLock<Regex> =
    std::sync::LazyLock::new(|| Regex::new(r"^(\d+)\s+(.+)").expect("valid regex"));

static CHOICE_LINE_REGEX: std::sync::LazyLock<Regex> =
    std::sync::LazyLock::new(|| Regex::new(r"^[a-e]\)\s*(.+)").expect("valid regex"));

pub fn find_question_anchors(text: &str) -> Vec<(usize, i32)> {
    QUESTION_ANCHOR_REGEX
        .captures_iter(text)
        .filter_map(|cap| {
            let num: i32 = cap[1].parse().ok()?;
            Some((cap.get(0)?.start(), num))
        })
        .collect()
}

pub fn split_questions_by_anchors(text: &str) -> (Vec<&str>, Option<Cow<'_, str>>) {
    let anchors = find_question_anchors(text);

    // If there are 0 or 1 anchors, we can't have a "complete" question
    // (since a complete question requires a start and an end anchor).
    if anchors.is_empty() {
        return (Vec::new(), None);
    }

    let mut questions = Vec::new();

    for pair in anchors.windows(2) {
        let start = pair[0].0;
        let end = pair[1].0;

        let question_text = text[start..end].trim();
        if question_text.len() < 50 {
            continue;
        }
        if !question_text.is_empty() {
            questions.push(question_text);
        }
    }

    // The text starting from the very last anchor to the end of the string
    // is incomplete because there is no following anchor to terminate it.
    let last_anchor_pos = anchors.last().unwrap().0;
    let trailing_text = text[last_anchor_pos..].trim();

    let incomplete = if trailing_text.is_empty() {
        None
    } else {
        Some(Cow::Borrowed(trailing_text))
    };

    (questions, incomplete)
}
