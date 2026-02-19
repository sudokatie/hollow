//! Export markdown documents to HTML format.

use pulldown_cmark::{html, Options, Parser};
use std::fs;
use std::io;
use std::path::Path;

/// Default CSS for exported HTML documents.
const DEFAULT_CSS: &str = r#"
body {
    max-width: 700px;
    margin: 40px auto;
    padding: 0 20px;
    font-family: Georgia, 'Times New Roman', serif;
    font-size: 18px;
    line-height: 1.6;
    color: #333;
}
h1, h2, h3, h4, h5, h6 {
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Helvetica, Arial, sans-serif;
    margin-top: 1.5em;
    margin-bottom: 0.5em;
}
h1 { font-size: 2em; }
h2 { font-size: 1.5em; }
h3 { font-size: 1.25em; }
code {
    font-family: 'SF Mono', Consolas, Monaco, monospace;
    font-size: 0.9em;
    background: #f4f4f4;
    padding: 0.2em 0.4em;
    border-radius: 3px;
}
pre {
    background: #f4f4f4;
    padding: 1em;
    overflow-x: auto;
    border-radius: 5px;
}
pre code {
    background: none;
    padding: 0;
}
blockquote {
    border-left: 4px solid #ddd;
    margin-left: 0;
    padding-left: 1em;
    color: #666;
}
a {
    color: #0366d6;
}
img {
    max-width: 100%;
}
hr {
    border: none;
    border-top: 1px solid #ddd;
    margin: 2em 0;
}
"#;

/// Export options.
#[derive(Default)]
pub struct ExportOptions {
    /// Custom CSS to use instead of default.
    pub css: Option<String>,
    /// Document title for HTML head.
    pub title: Option<String>,
}


/// Export a markdown file to HTML.
pub fn export_to_html<P: AsRef<Path>>(
    input: P,
    output: P,
    options: &ExportOptions,
) -> io::Result<()> {
    let markdown = fs::read_to_string(&input)?;
    let html_content = markdown_to_html(&markdown);
    
    let title = options.title.clone()
        .or_else(|| extract_title(&markdown))
        .unwrap_or_else(|| "Untitled".to_string());
    
    let css = options.css.as_deref().unwrap_or(DEFAULT_CSS);
    
    let full_html = format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{}</title>
    <style>{}</style>
</head>
<body>
{}
</body>
</html>
"#,
        html_escape(&title),
        css,
        html_content
    );
    
    fs::write(output, full_html)?;
    Ok(())
}

/// Convert markdown to HTML string.
pub fn markdown_to_html(markdown: &str) -> String {
    let options = Options::all();
    let parser = Parser::new_ext(markdown, options);
    
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);
    html_output
}

/// Extract title from first H1 heading in markdown.
fn extract_title(markdown: &str) -> Option<String> {
    for line in markdown.lines() {
        let trimmed = line.trim();
        if let Some(title) = trimmed.strip_prefix("# ") {
            return Some(title.trim().to_string());
        }
    }
    None
}

/// Escape HTML special characters.
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_markdown_to_html_basic() {
        let md = "# Hello\n\nThis is a paragraph.";
        let html = markdown_to_html(md);
        
        assert!(html.contains("<h1>Hello</h1>"));
        assert!(html.contains("<p>This is a paragraph.</p>"));
    }

    #[test]
    fn test_markdown_to_html_code() {
        let md = "Here is `inline code` and:\n\n```\ncode block\n```";
        let html = markdown_to_html(md);
        
        assert!(html.contains("<code>inline code</code>"));
        assert!(html.contains("<pre><code>"));
    }

    #[test]
    fn test_markdown_to_html_links() {
        let md = "Visit [example](https://example.com).";
        let html = markdown_to_html(md);
        
        assert!(html.contains("<a href=\"https://example.com\">example</a>"));
    }

    #[test]
    fn test_extract_title() {
        assert_eq!(extract_title("# My Title\n\nContent"), Some("My Title".to_string()));
        assert_eq!(extract_title("No heading here"), None);
        assert_eq!(extract_title("## Not H1"), None);
    }

    #[test]
    fn test_html_escape() {
        assert_eq!(html_escape("<script>"), "&lt;script&gt;");
        assert_eq!(html_escape("A & B"), "A &amp; B");
    }

    #[test]
    fn test_export_to_html() {
        let dir = tempdir().unwrap();
        let input = dir.path().join("test.md");
        let output = dir.path().join("test.html");
        
        fs::write(&input, "# Test\n\nHello world.").unwrap();
        
        export_to_html(&input, &output, &ExportOptions::default()).unwrap();
        
        let html = fs::read_to_string(&output).unwrap();
        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("<title>Test</title>"));
        assert!(html.contains("<h1>Test</h1>"));
        assert!(html.contains("<p>Hello world.</p>"));
    }

    #[test]
    fn test_export_with_custom_title() {
        let dir = tempdir().unwrap();
        let input = dir.path().join("test.md");
        let output = dir.path().join("test.html");
        
        fs::write(&input, "No heading\n\nJust text.").unwrap();
        
        let options = ExportOptions {
            title: Some("Custom Title".to_string()),
            css: None,
        };
        
        export_to_html(&input, &output, &options).unwrap();
        
        let html = fs::read_to_string(&output).unwrap();
        assert!(html.contains("<title>Custom Title</title>"));
    }

    #[test]
    fn test_default_css_included() {
        let dir = tempdir().unwrap();
        let input = dir.path().join("test.md");
        let output = dir.path().join("test.html");
        
        fs::write(&input, "# Test").unwrap();
        
        export_to_html(&input, &output, &ExportOptions::default()).unwrap();
        
        let html = fs::read_to_string(&output).unwrap();
        assert!(html.contains("max-width: 700px"));
        assert!(html.contains("font-family: Georgia"));
    }
}
