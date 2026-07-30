//! Snippet window extraction + query highlighting for search results.

/// Preferred snippet length (characters).
pub const SNIPPET_CHARS: usize = 220;
/// Characters of context kept on each side of a hit.
pub const SNIPPET_CONTEXT: usize = 80;

/// Build a highlighted snippet from `text` for `query`.
///
/// Prefers a contiguous match of the full query string, then falls back to a
/// window that covers as many query terms as possible.
pub fn make_snippet(text: &str, query: &str, case_sensitive: bool) -> String {
    if text.is_empty() {
        return String::new();
    }
    let terms = query_terms(query);
    let window = best_window(text, query, &terms, case_sensitive);
    highlight_terms(&window, query, &terms, case_sensitive)
}

/// Whether `text` already contains enough of the query to build a good snippet
/// without loading the full document body.
pub fn text_covers_query(text: &str, query: &str, case_sensitive: bool) -> bool {
    let q = query.trim();
    if q.is_empty() {
        return true;
    }
    if find_ci(text, q, case_sensitive).is_some() {
        return true;
    }
    let terms = query_terms(q);
    if terms.is_empty() {
        return false;
    }
    terms
        .iter()
        .all(|t| find_ci(text, t, case_sensitive).is_some())
}

/// Split a user query into highlight/match terms.
fn query_terms(query: &str) -> Vec<String> {
    let q = query.trim().trim_matches('"');
    if q.is_empty() {
        return Vec::new();
    }

    let mut terms = Vec::new();
    for raw in q.split_whitespace() {
        let w = raw.trim_matches(|c: char| matches!(c, '"' | '\'' | '“' | '”' | '‘' | '’'));
        if w.is_empty() {
            continue;
        }
        if w.chars().any(|c| c.is_ascii_alphanumeric()) {
            let cleaned: String = w
                .chars()
                .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-' || !c.is_ascii())
                .collect();
            if !cleaned.is_empty() {
                terms.push(cleaned);
            }
        } else {
            terms.push(w.to_string());
        }
    }

    if !q.contains(char::is_whitespace) && !terms.iter().any(|t| t == q) {
        terms.insert(0, q.to_string());
    }

    terms.sort_by_key(|t| std::cmp::Reverse(t.chars().count()));
    terms.dedup();
    terms
}

fn best_window(text: &str, query: &str, terms: &[String], case_sensitive: bool) -> String {
    let q = query.trim().trim_matches('"');
    if let Some(pos) = find_ci(text, q, case_sensitive) {
        return slice_window(text, pos, q.chars().count());
    }
    for term in terms {
        if let Some(pos) = find_ci(text, term, case_sensitive) {
            return slice_window(text, pos, term.chars().count());
        }
    }
    text.chars().take(SNIPPET_CHARS).collect()
}

fn slice_window(text: &str, match_char_pos: usize, match_char_len: usize) -> String {
    let total = text.chars().count();
    let start = match_char_pos.saturating_sub(SNIPPET_CONTEXT);
    let end = (match_char_pos + match_char_len + SNIPPET_CONTEXT).min(total);
    let mut s: String = text.chars().skip(start).take(end - start).collect();
    if start > 0 {
        s.insert_str(0, "…");
    }
    if end < total {
        s.push('…');
    }
    if s.chars().count() > SNIPPET_CHARS + 2 {
        s = s.chars().take(SNIPPET_CHARS).collect();
        s.push('…');
    }
    s
}

fn find_ci(hay: &str, needle: &str, case_sensitive: bool) -> Option<usize> {
    if needle.is_empty() {
        return None;
    }
    if case_sensitive {
        return hay.find(needle).map(|byte| hay[..byte].chars().count());
    }
    let hay_l = hay.to_lowercase();
    let needle_l = needle.to_lowercase();
    hay_l
        .find(&needle_l)
        .map(|byte| hay_l[..byte].chars().count())
}

fn highlight_terms(text: &str, query: &str, terms: &[String], case_sensitive: bool) -> String {
    let mut patterns: Vec<String> = Vec::new();
    let q = query.trim().trim_matches('"');
    if !q.is_empty() {
        patterns.push(q.to_string());
    }
    for t in terms {
        if !patterns.iter().any(|p| p == t) {
            patterns.push(t.clone());
        }
    }
    patterns.sort_by_key(|t| std::cmp::Reverse(t.chars().count()));

    let mut result = text.to_string();
    for term in &patterns {
        if term.chars().count() < 2 && term.is_ascii() {
            continue;
        }
        if term.is_empty() {
            continue;
        }
        result = wrap_all(&result, term, case_sensitive);
    }
    result
}

fn wrap_all(text: &str, term: &str, case_sensitive: bool) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while !rest.is_empty() {
        if let Some(stripped) = rest.strip_prefix("<b>") {
            if let Some(end) = stripped.find("</b>") {
                out.push_str("<b>");
                out.push_str(&stripped[..end]);
                out.push_str("</b>");
                rest = &stripped[end + 4..];
                continue;
            }
        }

        let search_in = if case_sensitive {
            rest.to_string()
        } else {
            rest.to_lowercase()
        };
        let search_for = if case_sensitive {
            term.to_string()
        } else {
            term.to_lowercase()
        };

        match search_in.find(&search_for) {
            None => {
                out.push_str(rest);
                break;
            }
            Some(byte_pos) => {
                let (before, after) = rest.split_at(byte_pos);
                out.push_str(before);
                let matched_len = term.len();
                let (matched, tail) = after.split_at(matched_len.min(after.len()));
                let ok = if case_sensitive {
                    matched == term
                } else {
                    matched.to_lowercase() == term.to_lowercase()
                };
                if ok {
                    out.push_str("<b>");
                    out.push_str(matched);
                    out.push_str("</b>");
                    rest = tail;
                } else {
                    let mut chars = after.chars();
                    if let Some(c) = chars.next() {
                        out.push(c);
                        rest = chars.as_str();
                    } else {
                        break;
                    }
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefers_full_phrase_window() {
        let body = format!(
            "{}{}",
            "大模型参数高效微调方法综述: 技术、趋势与挑战\n",
            "x".repeat(400)
        ) + "时对独立参数采用非结构化微调，对高敏感参数密集的权重矩阵采用结构化微调。";
        let snip = make_snippet(&body, "独立参数", false);
        assert!(snip.contains("独立参数") || snip.contains("<b>独立参数</b>"));
        assert!(snip.contains("<b>"));
        assert!(!snip.starts_with("大模型参数高效"));
    }

    #[test]
    fn highlights_phrase_not_single_chars() {
        let text = "时对独立参数采用非结构化微调";
        let snip = make_snippet(text, "独立参数", false);
        assert!(snip.contains("<b>独立参数</b>"));
        assert!(!snip.contains("<b>参</b>"));
    }

    #[test]
    fn covers_query_requires_phrase() {
        let title = "大模型参数高效微调方法综述";
        assert!(!text_covers_query(title, "独立参数", false));
        let body = "时对独立参数采用非结构化微调";
        assert!(text_covers_query(body, "独立参数", false));
    }

    #[test]
    fn query_terms_keep_cjk_phrase() {
        let terms = query_terms("独立参数");
        assert_eq!(terms[0], "独立参数");
    }
}
