//! Markdown formatter.

use crate::util::unwrap;
use crate::ManMap;
use anyhow::{bail, format_err, Error};
use std::fmt::Write;

pub struct MdFormatter {
    man_map: ManMap,
}

impl MdFormatter {
    pub fn new(man_map: ManMap) -> MdFormatter {
        MdFormatter { man_map }
    }
}

impl MdFormatter {
    fn render_html(&self, input: &str) -> Result<String, Error> {
        let parser = crate::md_parser(input, None);
        let mut html_output: String = String::with_capacity(input.len() * 3 / 2);
        pulldown_cmark::html::push_html(&mut html_output, parser.map(|(e, _r)| e));
        Ok(html_output)
    }
}

impl super::Formatter for MdFormatter {
    fn render(&self, input: &str) -> Result<String, Error> {
        Ok(input.replace("\r\n", "\n"))
    }

    fn render_options_start(&self) -> &'static str {
        "<dl>"
    }

    fn render_options_end(&self) -> &'static str {
        "</dl>"
    }

    fn render_option(&self, params: &[&str], block: &str, man_name: &str) -> Result<String, Error> {
        let mut result = String::new();
        fn unwrap_p(t: &str) -> &str {
            unwrap(t, "<p>", "</p>")
        }

        for param in params {
            let rendered = self.render_html(param)?;
            let no_p = unwrap_p(&rendered);
            // split out first term to use as the id.
            let first = no_p
                .split_whitespace()
                .next()
                .ok_or_else(|| format_err!("did not expect option `{}` to be empty", param))?;
            let no_tags = trim_tags(first);
            if no_tags.is_empty() {
                bail!("unexpected empty option with no tags `{}`", param);
            }
            let id = format!("option-{}-{}", man_name, no_tags);
            write!(
                result,
                "<dt class=\"option-term\" id=\"{ID}\">\
                <a class=\"option-anchor\" href=\"#{ID}\"></a>{OPTION}</dt>\n",
                ID = id,
                OPTION = no_p
            )?;
        }
        let rendered_block = self.render_html(block)?;
        write!(
            result,
            "<dd class=\"option-desc\">{}</dd>\n",
            unwrap_p(&rendered_block)
        )?;
        Ok(result)
    }

    fn linkify_man_to_md(&self, name: &str, section: u8) -> Result<String, Error> {
        let s = match self.man_map.get(&(name.to_string(), section)) {
            Some(link) => format!("[{}({})]({})", name, section, link),
            None => format!("[{}({})]({}.html)", name, section, name),
        };
        Ok(s)
    }
}

fn trim_tags(s: &str) -> String {
    // This is a hack. It removes all HTML tags.
    let mut in_tag = false;
    let mut in_char_ref = false;
    s.chars()
        .filter(|&ch| match ch {
            '<' if in_tag => panic!("unexpected nested tag"),
            '&' if in_char_ref => panic!("unexpected nested char ref"),
            '<' => {
                in_tag = true;
                false
            }
            '&' => {
                in_char_ref = true;
                false
            }
            '>' if in_tag => {
                in_tag = false;
                false
            }
            ';' if in_char_ref => {
                in_char_ref = false;
                false
            }
            _ => !in_tag && !in_char_ref,
        })
        .collect()
}
