use pulldown_cmark::{html, CowStr, Event, Options, Parser};

/// Renders a note to sanitized HTML. Task checkboxes become clickable
/// `<input type="checkbox" data-line="N">`, where N is the 0-based source line.
pub fn render(body: &str) -> String {
    let opts = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;
    let line_starts: Vec<usize> = std::iter::once(0)
        .chain(body.match_indices('\n').map(|(i, _)| i + 1))
        .collect();

    let events = Parser::new_ext(body, opts)
        .into_offset_iter()
        .map(|(event, range)| match event {
            Event::TaskListMarker(checked) => {
                let line = line_starts.partition_point(|&s| s <= range.start) - 1;
                let checked = if checked { " checked" } else { "" };
                Event::Html(CowStr::from(format!(
                    r#"<input type="checkbox" data-line="{line}"{checked}>"#
                )))
            }
            other => other,
        });

    let mut raw = String::with_capacity(body.len() * 3 / 2);
    html::push_html(&mut raw, events);

    ammonia::Builder::default()
        .add_tags(&["input"])
        .add_tag_attributes("input", &["type", "checked", "data-line"])
        .clean(&raw)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_checkboxes_carry_line_numbers() {
        let html = render("Title\n\n- [ ] one\n- [x] two");
        assert!(html.contains(r#"data-line="2""#), "{html}");
        assert!(html.contains(r#"data-line="3""#), "{html}");
        assert!(html.contains("checked"), "{html}");
    }

    #[test]
    fn strips_scripts() {
        let html = render("<script>alert(1)</script><img src=x onerror=alert(1)>");
        assert!(!html.contains("<script"));
        assert!(!html.contains("onerror"));
    }
}
