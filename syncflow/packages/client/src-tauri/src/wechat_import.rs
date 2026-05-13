use chrono::{DateTime, Utc};
use std::borrow::Cow;

const MAX_CLIPBOARD_CHARS: usize = 2_000_000;

#[derive(Debug, Clone)]
pub struct WeChatClipboardPayload {
    pub html: Option<String>,
    pub text: Option<String>,
    pub source_url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ParsedWeChatArticle {
    pub title: String,
    pub account: Option<String>,
    pub author: Option<String>,
    pub published_at: Option<String>,
    pub original_url: Option<String>,
    pub markdown: String,
    pub image_urls: Vec<String>,
}

pub fn parse_wechat_clipboard(
    payload: WeChatClipboardPayload,
    imported_at: DateTime<Utc>,
) -> Result<ParsedWeChatArticle, String> {
    let html = payload
        .html
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let text = payload
        .text
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());

    if html.map(str::len).unwrap_or(0) + text.map(str::len).unwrap_or(0) > MAX_CLIPBOARD_CHARS {
        return Err("Clipboard content is too large to import".to_string());
    }

    let mut article = if let Some(html) = html {
        parse_html_article(html)?
    } else if let Some(text) = text {
        parse_text_article(text)?
    } else {
        return Err("Clipboard is empty. Copy the article content first.".to_string());
    };

    if article.original_url.is_none() {
        article.original_url = payload.source_url.filter(|value| !value.trim().is_empty());
    }

    article.markdown = render_front_matter(&article, imported_at, "clipboard") + &article.markdown;
    Ok(article)
}

pub fn safe_article_file_name(title: &str) -> String {
    let mut result = String::new();
    let mut previous_space = false;
    for ch in title.trim().chars() {
        let replacement = if ch.is_control()
            || matches!(ch, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|')
        {
            ' '
        } else {
            ch
        };
        if replacement.is_whitespace() {
            if !previous_space && !result.is_empty() {
                result.push(' ');
            }
            previous_space = true;
        } else {
            result.push(replacement);
            previous_space = false;
        }
        if result.chars().count() >= 80 {
            break;
        }
    }

    let trimmed = result.trim_matches([' ', '.']).to_string();
    let reserved = [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    if trimmed.is_empty()
        || reserved
            .iter()
            .any(|value| trimmed.eq_ignore_ascii_case(value))
    {
        "WeChat Article".to_string()
    } else {
        trimmed
    }
}

fn parse_html_article(html: &str) -> Result<ParsedWeChatArticle, String> {
    let cleaned = strip_ignored_blocks(html);
    let title = extract_meta_content(&cleaned, "og:title")
        .or_else(|| extract_between_attr(&cleaned, "id", "activity-name"))
        .or_else(|| extract_title_tag(&cleaned))
        .map(|value| normalize_text(&html_to_text(&value)))
        .filter(|value| !value.is_empty());
    let account = extract_between_attr(&cleaned, "id", "js_name")
        .or_else(|| extract_meta_content(&cleaned, "og:article:author"))
        .map(|value| normalize_text(&html_to_text(&value)))
        .filter(|value| !value.is_empty());
    let author = extract_between_attr(&cleaned, "id", "js_author_name")
        .map(|value| normalize_text(&html_to_text(&value)))
        .filter(|value| !value.is_empty());
    let published_at = extract_between_attr(&cleaned, "id", "publish_time")
        .map(|value| normalize_text(&html_to_text(&value)))
        .filter(|value| !value.is_empty());
    let original_url = extract_meta_content(&cleaned, "og:url")
        .or_else(|| extract_first_url(&cleaned))
        .filter(|value| !value.is_empty());

    let body_html = extract_between_attr(&cleaned, "id", "js_content")
        .or_else(|| extract_between_attr(&cleaned, "class", "rich_media_content"))
        .unwrap_or_else(|| cleaned.clone());
    let image_urls = extract_image_urls(&body_html);
    let markdown = html_to_markdown(&body_html);
    let markdown = collapse_blank_lines(&markdown);
    if markdown.trim().is_empty() {
        return Err("Clipboard content does not contain readable article text".to_string());
    }

    let title = title.unwrap_or_else(|| {
        first_meaningful_line(&markdown).unwrap_or_else(|| "WeChat Article".to_string())
    });
    Ok(ParsedWeChatArticle {
        title,
        account,
        author,
        published_at,
        original_url,
        markdown,
        image_urls,
    })
}

fn parse_text_article(text: &str) -> Result<ParsedWeChatArticle, String> {
    let markdown = collapse_blank_lines(text.trim());
    if markdown.is_empty() {
        return Err("Clipboard text is empty".to_string());
    }
    let title = first_meaningful_line(&markdown).unwrap_or_else(|| "WeChat Article".to_string());
    Ok(ParsedWeChatArticle {
        title,
        account: None,
        author: None,
        published_at: None,
        original_url: extract_first_url(text),
        markdown,
        image_urls: Vec::new(),
    })
}

fn render_front_matter(
    article: &ParsedWeChatArticle,
    imported_at: DateTime<Utc>,
    import_method: &str,
) -> String {
    let mut lines = vec![
        "---".to_string(),
        "source: wechat".to_string(),
        format!("title: \"{}\"", yaml_escape(&article.title)),
    ];
    if let Some(account) = article.account.as_deref() {
        lines.push(format!("account: \"{}\"", yaml_escape(account)));
    }
    if let Some(author) = article.author.as_deref() {
        lines.push(format!("author: \"{}\"", yaml_escape(author)));
    }
    if let Some(published_at) = article.published_at.as_deref() {
        lines.push(format!("published_at: \"{}\"", yaml_escape(published_at)));
    }
    if let Some(original_url) = article.original_url.as_deref() {
        lines.push(format!("original_url: \"{}\"", yaml_escape(original_url)));
    }
    lines.push(format!("imported_at: \"{}\"", imported_at.to_rfc3339()));
    lines.push(format!("import_method: \"{}\"", yaml_escape(import_method)));
    lines.push("---".to_string());
    lines.push(String::new());
    lines.join("\n")
}

fn html_to_markdown(html: &str) -> String {
    let mut out = String::new();
    let mut index = 0usize;
    while let Some(relative_start) = html[index..].find('<') {
        let start = index + relative_start;
        push_text(&mut out, &html[index..start]);
        let Some(relative_end) = html[start..].find('>') else {
            push_text(&mut out, &html[start..]);
            return out;
        };
        let end = start + relative_end;
        let tag = &html[start + 1..end];
        handle_tag(&mut out, tag);
        index = end + 1;
    }
    push_text(&mut out, &html[index..]);
    out
}

fn handle_tag(out: &mut String, tag: &str) {
    let tag = tag.trim();
    if tag.is_empty() || tag.starts_with('!') {
        return;
    }
    let closing = tag.starts_with('/');
    let normalized = tag
        .trim_start_matches('/')
        .split_whitespace()
        .next()
        .unwrap_or("")
        .trim_end_matches('/')
        .to_ascii_lowercase();
    match normalized.as_str() {
        "h1" if !closing => out.push_str("\n# "),
        "h2" if !closing => out.push_str("\n## "),
        "h3" if !closing => out.push_str("\n### "),
        "h4" if !closing => out.push_str("\n#### "),
        "h5" if !closing => out.push_str("\n##### "),
        "h6" if !closing => out.push_str("\n###### "),
        "p" | "section" | "article" | "div" | "blockquote" if !closing => out.push_str("\n\n"),
        "br" => out.push('\n'),
        "li" if !closing => out.push_str("\n- "),
        "img" if !closing => {
            if let Some(src) =
                extract_tag_attr(tag, "data-src").or_else(|| extract_tag_attr(tag, "src"))
            {
                out.push_str(&format!("![image]({})", src));
            }
        }
        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "p" | "section" | "article" | "div"
        | "blockquote" | "ul" | "ol" => out.push_str("\n\n"),
        _ => {}
    }
}

fn extract_image_urls(html: &str) -> Vec<String> {
    let mut urls = Vec::new();
    let lower = html.to_ascii_lowercase();
    let mut search_start = 0usize;
    while let Some(relative_start) = lower[search_start..].find("<img") {
        let start = search_start + relative_start;
        let Some(relative_end) = lower[start..].find('>') else {
            break;
        };
        let end = start + relative_end;
        let tag = &html[start + 1..end];
        if let Some(src) =
            extract_tag_attr(tag, "data-src").or_else(|| extract_tag_attr(tag, "src"))
        {
            if (src.starts_with("https://") || src.starts_with("http://"))
                && !urls.iter().any(|existing| existing == &src)
            {
                urls.push(src);
            }
        }
        search_start = end + 1;
    }
    urls
}

fn push_text(out: &mut String, text: &str) {
    let decoded = decode_html_entities(text);
    let normalized = normalize_inline_text(&decoded);
    if normalized.is_empty() {
        return;
    }
    if out.ends_with('\n') {
        out.push_str(normalized.trim_start());
    } else {
        out.push_str(&normalized);
    }
}

fn strip_ignored_blocks(html: &str) -> String {
    let mut value = html.to_string();
    for tag in ["script", "style", "noscript", "svg"] {
        value = strip_tag_pair(&value, tag);
    }
    value
}

fn strip_tag_pair(html: &str, tag: &str) -> String {
    let mut result = String::new();
    let mut rest = html;
    let open_pattern = format!("<{tag}");
    let close_pattern = format!("</{tag}>");
    loop {
        let lower = rest.to_ascii_lowercase();
        let Some(start) = lower.find(&open_pattern) else {
            result.push_str(rest);
            break;
        };
        result.push_str(&rest[..start]);
        let after_start = &rest[start..];
        let after_lower = after_start.to_ascii_lowercase();
        if let Some(close) = after_lower.find(&close_pattern) {
            rest = &after_start[close + close_pattern.len()..];
        } else {
            break;
        }
    }
    result
}

fn extract_between_attr(html: &str, attr_name: &str, attr_value: &str) -> Option<String> {
    let lower = html.to_ascii_lowercase();
    let needles = [
        format!("{attr_name}=\"{}\"", attr_value.to_ascii_lowercase()),
        format!("{attr_name}='{}'", attr_value.to_ascii_lowercase()),
    ];
    let start_attr = needles
        .iter()
        .filter_map(|needle| lower.find(needle))
        .min()?;
    let tag_start = lower[..start_attr].rfind('<')?;
    let tag_end = lower[tag_start..].find('>')? + tag_start;
    let tag_name = lower[tag_start + 1..tag_end]
        .split_whitespace()
        .next()
        .unwrap_or("")
        .trim_matches('/');
    if tag_name.is_empty() {
        return None;
    }
    let close = format!("</{tag_name}>");
    let content_start = tag_end + 1;
    let content_end = lower[content_start..].find(&close)? + content_start;
    Some(html[content_start..content_end].to_string())
}

fn extract_meta_content(html: &str, property: &str) -> Option<String> {
    let lower = html.to_ascii_lowercase();
    let property_lower = property.to_ascii_lowercase();
    let mut search_start = 0usize;
    while let Some(relative_start) = lower[search_start..].find("<meta") {
        let start = search_start + relative_start;
        let Some(relative_end) = lower[start..].find('>') else {
            break;
        };
        let end = start + relative_end;
        let tag = &html[start + 1..end];
        let tag_lower = tag.to_ascii_lowercase();
        if tag_lower.contains(&format!("property=\"{property_lower}\""))
            || tag_lower.contains(&format!("property='{property_lower}'"))
            || tag_lower.contains(&format!("name=\"{property_lower}\""))
            || tag_lower.contains(&format!("name='{property_lower}'"))
        {
            return extract_tag_attr(tag, "content")
                .map(|value| decode_html_entities(&value).into_owned());
        }
        search_start = end + 1;
    }
    None
}

fn extract_title_tag(html: &str) -> Option<String> {
    let lower = html.to_ascii_lowercase();
    let start = lower.find("<title")?;
    let tag_end = lower[start..].find('>')? + start;
    let content_start = tag_end + 1;
    let content_end = lower[content_start..].find("</title>")? + content_start;
    Some(html[content_start..content_end].to_string())
}

fn extract_tag_attr(tag: &str, attr_name: &str) -> Option<String> {
    let lower = tag.to_ascii_lowercase();
    for quote in ['"', '\''] {
        let needle = format!("{attr_name}={quote}");
        if let Some(start) = lower.find(&needle) {
            let value_start = start + needle.len();
            let rest = &tag[value_start..];
            let end = rest.find(quote)?;
            return Some(decode_html_entities(&rest[..end]).into_owned());
        }
    }
    None
}

fn extract_first_url(value: &str) -> Option<String> {
    value
        .split_whitespace()
        .find(|part| {
            part.starts_with("https://mp.weixin.qq.com/")
                || part.starts_with("http://mp.weixin.qq.com/")
        })
        .map(|part| {
            part.trim_matches(['"', '\'', ')', ']', '>', '<'])
                .to_string()
        })
}

fn html_to_text(html: &str) -> String {
    collapse_blank_lines(&html_to_markdown(html))
}

fn normalize_text(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn normalize_inline_text(value: &str) -> String {
    value
        .replace('\u{00a0}', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn collapse_blank_lines(value: &str) -> String {
    let mut lines = Vec::new();
    let mut previous_blank = true;
    for line in value.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if !previous_blank {
                lines.push(String::new());
            }
            previous_blank = true;
        } else {
            lines.push(trimmed.to_string());
            previous_blank = false;
        }
    }
    while matches!(lines.last(), Some(line) if line.is_empty()) {
        lines.pop();
    }
    lines.join("\n")
}

fn first_meaningful_line(value: &str) -> Option<String> {
    value
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with("!["))
        .map(|line| line.trim_start_matches('#').trim().to_string())
}

fn yaml_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn decode_html_entities(value: &str) -> Cow<'_, str> {
    if !value.contains('&') {
        return Cow::Borrowed(value);
    }
    Cow::Owned(
        value
            .replace("&nbsp;", " ")
            .replace("&amp;", "&")
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&quot;", "\"")
            .replace("&#39;", "'")
            .replace("&apos;", "'"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_wechat_html_clipboard() {
        let html = r#"
            <html><head>
              <meta property="og:title" content="A useful article">
              <meta property="og:url" content="https://mp.weixin.qq.com/s/example">
            </head><body>
              <h1 id="activity-name">A useful article</h1>
              <span id="js_name">SyncFlow Notes</span>
              <div id="js_content"><p>Hello &amp; welcome</p><p><img data-src="https://example.com/a.jpg"></p></div>
            </body></html>
        "#;
        let article = parse_wechat_clipboard(
            WeChatClipboardPayload {
                html: Some(html.to_string()),
                text: None,
                source_url: None,
            },
            Utc::now(),
        )
        .unwrap();

        assert_eq!(article.title, "A useful article");
        assert_eq!(article.account.as_deref(), Some("SyncFlow Notes"));
        assert!(article.markdown.contains("Hello & welcome"));
        assert!(article
            .markdown
            .contains("![image](https://example.com/a.jpg)"));
    }

    #[test]
    fn text_clipboard_uses_first_line_as_title() {
        let article = parse_wechat_clipboard(
            WeChatClipboardPayload {
                html: None,
                text: Some("Title\n\nBody".to_string()),
                source_url: None,
            },
            Utc::now(),
        )
        .unwrap();

        assert_eq!(article.title, "Title");
        assert!(article.markdown.contains("Body"));
    }

    #[test]
    fn safe_file_name_removes_windows_reserved_characters() {
        assert_eq!(safe_article_file_name(" a/b:c*? \"x\" "), "a b c x");
        assert_eq!(safe_article_file_name("CON"), "WeChat Article");
    }
}
