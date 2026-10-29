// Parser regression tests are kept out of md.rs so production parsing code stays readable.
use super::*;

#[cfg(test)]
mod core_tests {
    use super::*;

    #[test]
    fn fragment_spaces_punctuation_unicode_use_underscore() {
        let id = slugify("Első lépések: TCP/IP?");
        assert_eq!(id, "első_lépések_tcp_ip");
        assert_eq!(percent_decode_fragment(&percent_encode_fragment(&id)), id);
    }

    #[test]
    fn raw_encoded_and_old_hyphen_fragment_match() {
        let a = canonicalize_url_fragment("https://e/x.md#Első lépések: TCP/IP?");
        let b = canonicalize_url_fragment("https://e/x.md#Els%C5%91%20l%C3%A9p%C3%A9sek%3A%20TCP%2FIP%3F");
        let c = canonicalize_url_fragment("https://e/x.md#els%C5%91-l%C3%A9p%C3%A9sek-tcp-ip");
        assert_eq!(a, b);
        assert_eq!(a, c);
        assert_eq!(a, "https://e/x.md#els%C5%91_l%C3%A9p%C3%A9sek_tcp_ip");
    }

    #[test]
    fn duplicate_headings_get_unique_ids() {
        let d = parse("## Same\n## Same\n## Same");
        assert_eq!(d.lines[0].anchor.as_deref(), Some("same"));
        assert_eq!(d.lines[1].anchor.as_deref(), Some("same_1"));
        assert_eq!(d.lines[2].anchor.as_deref(), Some("same_2"));
    }

    #[test]
    fn nested_list_indent_is_kept() {
        let d = parse("- one\n  - two\n    - three");
        assert_eq!(d.lines[0].indent, 0);
        assert_eq!(d.lines[1].indent, 1);
        assert_eq!(d.lines[2].indent, 2);
    }

    #[test]
    fn image_is_detected() {
        let d = parse("![logo](images/logo.png)");
        assert!(matches!(d.lines[0].style, Style::Image));
        assert_eq!(d.lines[0].image.as_ref().unwrap().target, "images/logo.png");
    }

    #[test]
    fn image_title_and_angle_destination_are_accepted() {
        let d = parse("![photo](../img/a.jpg \"A photo\")\n![spaced](<../img/my photo.jpg>)");
        assert_eq!(d.lines[0].image.as_ref().unwrap().target, "../img/a.jpg");
        assert_eq!(d.lines[1].image.as_ref().unwrap().target, "../img/my photo.jpg");
    }

    #[test]
    fn table_rows_are_detected() {
        let d = parse("| A | B |\n|---|---|\n| 1 | 2 |");
        assert!(matches!(d.lines[0].style, Style::TableRow));
        assert!(matches!(d.lines[1].style, Style::TableSep));
        assert!(matches!(d.lines[2].style, Style::TableRow));
    }

    #[test]
    fn ordered_list_is_detected() {
        let d = parse("1. first\n12. twelfth");
        assert!(matches!(d.lines[0].style, Style::Ordered(1)));
        assert!(matches!(d.lines[1].style, Style::Ordered(12)));
    }

    #[test]
    fn h4_gets_anchor() {
        let d = parse("#### Deep section");
        assert!(matches!(d.lines[0].style, Style::H4));
        assert_eq!(d.lines[0].anchor.as_deref(), Some("deep_section"));
    }

    #[test]
    fn strikethrough_uses_pango_markup() {
        let d = parse("before ~~old~~ after");
        assert!(d.lines[0].markup.contains("strikethrough=\"true\""));
    }

    #[test]
    fn backslash_escapes_markdown_punctuation() {
        let d = parse(r"\*literal asterisks\* and \[brackets\]");
        assert!(d.lines[0].markup.contains("*literal asterisks*"));
        assert!(d.lines[0].markup.contains("[brackets]"));
    }

    #[test]
    fn raw_html_is_rendered_as_text_not_markup() {
        let d = parse("<script>alert(1)</script>");
        assert!(d.lines[0].markup.contains("&lt;script&gt;"));
        assert!(!d.lines[0].markup.contains("<script>"));
    }

    #[test]
    fn inline_math_is_rendered() {
        let d = parse(r"Energy $E=mc^2$");
        assert!(d.lines[0].markup.contains("rise=\"7000\""));
    }
    #[test]
    fn display_math_is_a_math_line() {
        let d = parse("$$\n\\frac{a}{b}\n$$");
        assert!(matches!(d.lines[0].style, Style::Math));
        assert!(d.lines[0].markup.contains('⁄'));
    }

    #[test]
    fn github_safe_inline_math_is_rendered() {
        let d = parse(r"Safe $`\sqrt{3x-1}+(1+x)^2`$ inline");
        assert!(d.lines[0].markup.contains('√'));
        assert!(d.lines[0].markup.contains("rise=\"7000\""));
    }

    #[test]
    fn github_fenced_math_is_rendered() {
        let d = parse("```math\n\\sum_{i=1}^{n} i\n```");
        assert!(matches!(d.lines[0].style, Style::Math));
        assert!(d.lines[0].markup.contains('∑'));
    }

    #[test]
    fn ordinary_code_fence_stays_code() {
        let d = parse("```rust\nlet x = 1;\n```");
        assert!(d.lines.iter().any(|l| matches!(l.style, Style::Code)));
    }

    #[test]
    fn code_fence_keeps_language_and_text() {
        let d = parse("```rust\nfn main() {}\n```");
        let c = d.lines[0].code.as_ref().unwrap();
        assert_eq!(c.language, "rust");
        assert_eq!(c.text, "fn main() {}");
    }

    #[test]
    fn markdown_link_title_is_kept() {
        let d = parse("[site](https://example.org \"Example site\")");
        assert_eq!(d.links[0].title.as_deref(), Some("Example site"));
        assert_eq!(d.lines[0].tooltip.as_deref(), Some("Example site"));
    }

    #[test]
    fn angle_and_bare_http_are_autolinks() {
        let d = parse("<http://example.org:8080/a> and http://example.org:8080/b.");
        assert_eq!(d.links.len(), 2);
        assert_eq!(d.links[0].target, "http://example.org:8080/a");
        assert_eq!(d.links[1].target, "http://example.org:8080/b");
    }

    #[test]
    fn angle_and_bare_https_are_autolinks() {
        let d = parse("<https://example.org/a> and https://example.org/b.");
        assert_eq!(d.links.len(), 2);
        assert_eq!(d.links[0].target, "https://example.org/a");
        assert_eq!(d.links[1].target, "https://example.org/b");
    }

    #[test]
    fn h5_h6_get_anchors() {
        let d = parse("##### Five\n###### Six");
        assert!(matches!(d.lines[0].style, Style::H5));
        assert!(matches!(d.lines[1].style, Style::H6));
        assert_eq!(d.lines[0].anchor.as_deref(), Some("five"));
        assert_eq!(d.lines[1].anchor.as_deref(), Some("six"));
    }

    #[test]
    fn tilde_code_and_math_fences_work() {
        let d = parse("~~~rust\nlet x = 1;\n~~~\n~~~math\n\\sqrt{x}\n~~~");
        assert!(matches!(d.lines[0].style, Style::Code));
        assert_eq!(d.lines[0].code.as_ref().unwrap().language, "rust");
        assert!(d.lines.iter().any(|l| matches!(l.style, Style::Math)));
    }

    #[test]
    fn plus_lists_and_spaced_thematic_breaks_work() {
        let d = parse("+ item\n+ [x] done\n- - -");
        assert!(matches!(d.lines[0].style, Style::Bullet));
        assert!(matches!(d.lines[1].style, Style::Task(true)));
        assert!(matches!(d.lines[2].style, Style::Rule));
    }
}

#[cfg(test)]
mod completeness_tests {
    use super::*;
    #[test]
    fn html_entities_decode() {
        let (m, _) = parse_inline("A &amp; B &#169; &#x03B1;");
        assert!(m.contains("A &amp; B © α"));
    }
    #[test]
    fn footnotes_are_collected_and_linked() {
        let d = parse("Text[^note]\n\n[^note]: Footnote body");
        assert!(d.lines.iter().any(|l| l.anchor.as_deref() == Some("fn_note")));
        assert!(d.lines.iter().any(|l| l.markup.contains("#fn_note")));
    }
    #[test]
    fn nested_quote_depth() {
        let d = parse("> > nested");
        assert!(matches!(d.lines[0].style, Style::Quote));
        assert_eq!(d.lines[0].indent, 1);
    }
    #[test]
    fn soft_and_hard_breaks() {
        let soft = parse("one\ntwo");
        assert_eq!(soft.lines.len(), 1);
        assert!(soft.lines[0].markup.contains("one two"));
        let hard = parse("one  \ntwo");
        assert_eq!(hard.lines.len(), 1);
        assert!(hard.lines[0].markup.contains("one  \ntwo"));
    }
    #[test]
    fn table_alignment_is_parsed() {
        let d = parse("| left | center | right |\n| :--- | :---: | ---: |\n| a | b | c |");
        let sep = d.lines.iter().find(|l| matches!(l.style, Style::TableSep)).unwrap();
        let a = &sep.table.as_ref().unwrap().align;
        assert_eq!(a, &vec![TableAlign::Left, TableAlign::Center, TableAlign::Right]);
    }
}

#[cfg(test)]
mod edge_case_tests {
    use super::*;
    #[test]
    fn setext_headings_are_supported() {
        let d = parse("Title\n=====\nSubtitle\n-----");
        assert!(matches!(d.lines[0].style, Style::H1));
        assert!(matches!(d.lines[1].style, Style::H2));
        assert_eq!(d.lines[0].anchor.as_deref(), Some("title"));
        assert_eq!(d.lines[1].anchor.as_deref(), Some("subtitle"));
    }
    #[test]
    fn setext_markers_inside_fence_are_not_headings() {
        let d = parse("```text\nTitle\n=====\n```");
        assert_eq!(d.lines.len(), 1);
        assert!(matches!(d.lines[0].style, Style::Code));
    }
    #[test]
    fn multi_backtick_inline_code_keeps_inner_tick() {
        let d = parse("Use ``a ` b`` here");
        assert!(d.lines[0].markup.contains("<tt>a ` b</tt>"));
    }
    #[test]
    fn fence_close_must_match_character_and_minimum_length() {
        let d = parse("````rust\n```\nlet x = 1;\n````");
        assert_eq!(d.lines.len(), 1);
        let c = d.lines[0].code.as_ref().unwrap();
        assert!(c.text.contains("```"));
        let d2 = parse("~~~text\n```\n~~~");
        assert_eq!(d2.lines[0].code.as_ref().unwrap().text, "```");
    }
    #[test]
    fn lazy_list_continuation_is_joined() {
        let d = parse("- first line\ncontinuation\n\nplain paragraph");
        assert!(matches!(d.lines[0].style, Style::Bullet));
        assert!(d.lines[0].markup.contains("first line continuation"));
        assert!(matches!(d.lines.last().unwrap().style, Style::Text));
    }

    #[test]
    fn reference_links_full_collapsed_and_shortcut_work() {
        let d = parse(
            "[Rust][rust-site] [Docs][] [Home]\n\n[rust-site]: https://www.rust-lang.org/ \"Rust\"\n[docs]: https://doc.rust-lang.org/\n[home]: https://example.org/",
        );
        assert_eq!(d.links.len(), 3);
        assert_eq!(d.links[0].target, "https://www.rust-lang.org/");
        assert_eq!(d.links[0].title.as_deref(), Some("Rust"));
        assert_eq!(d.links[1].target, "https://doc.rust-lang.org/");
        assert_eq!(d.links[2].target, "https://example.org/");
    }

    #[test]
    fn reference_labels_are_case_and_space_insensitive() {
        let d = parse("[x][A   B]\n\n[a b]: https://example.org/");
        assert_eq!(d.links[0].target, "https://example.org/");
    }

    #[test]
    fn reference_images_work() {
        let d = parse("![Logo][asset]\n\n[asset]: images/logo.png \"Logo image\"");
        let img = d.lines.iter().find_map(|l| l.image.as_ref()).unwrap();
        assert_eq!(img.target, "images/logo.png");
        assert_eq!(img.title.as_deref(), Some("Logo image"));
    }

    #[test]
    fn nested_emphasis_and_strong_are_rendered() {
        let d = parse("**bold *italic* text**");
        assert!(d.lines[0].markup.contains("<b>"));
        assert!(d.lines[0].markup.contains("<i>italic</i>"));
    }

    #[test]
    fn reference_definition_inside_code_fence_is_not_consumed() {
        let d = parse("```text\n[x]: https://example.org/\n```\n[x]");
        assert_eq!(d.lines[0].code.as_ref().unwrap().text, "[x]: https://example.org/");
        assert!(d.links.is_empty());
    }

    #[test]
    fn inline_link_destination_accepts_balanced_parentheses() {
        let d = parse("[spec](docs/api_(v2).md#request)");
        assert_eq!(d.links.len(), 1);
        assert_eq!(d.links[0].target, "docs/api_(v2).md#request");
    }

    #[test]
    fn image_destination_accepts_balanced_parentheses() {
        let d = parse("![plot](images/result_(final).png)");
        let img = d.lines.iter().find_map(|l| l.image.as_ref()).unwrap();
        assert_eq!(img.target, "images/result_(final).png");
    }

    #[test]
    fn lazy_quote_continuation_is_joined() {
        let d = parse("> quoted line\ncontinuation");
        assert_eq!(d.lines.len(), 1);
        assert!(matches!(d.lines[0].style, Style::Quote));
        assert!(d.lines[0].markup.contains("quoted line continuation"));
    }
}

#[cfg(test)]
mod nested_block_v034_tests {
    use super::*;

    #[test]
    fn list_inside_blockquote_keeps_list_semantics() {
        let d = parse("> - quoted bullet\n> 3. quoted ordered\n> - [x] quoted task");
        assert!(matches!(d.lines[0].style, Style::Bullet));
        assert!(matches!(d.lines[1].style, Style::Ordered(3)));
        assert!(matches!(d.lines[2].style, Style::Task(true)));
    }

    #[test]
    fn nested_lists_keep_increasing_indent() {
        let d = parse("- outer\n  - inner\n    7. deeper");
        assert!(matches!(d.lines[0].style, Style::Bullet));
        assert!(matches!(d.lines[1].style, Style::Bullet));
        assert!(matches!(d.lines[2].style, Style::Ordered(7)));
        assert!(d.lines[0].indent < d.lines[1].indent);
        assert!(d.lines[1].indent < d.lines[2].indent);
    }

    #[test]
    fn fenced_code_under_list_preserves_indent() {
        let d = parse("- item\n  ```rust\n  let x = 1;\n  ```");
        let code = d.lines.iter().find(|l| matches!(l.style, Style::Code)).unwrap();
        assert_eq!(code.indent, 1);
        assert_eq!(code.code.as_ref().unwrap().language, "rust");
    }

    #[test]
    fn blank_line_ends_lazy_list_continuation() {
        let d = parse("- item\ncontinuation\n\nnew paragraph");
        assert!(d.lines[0].markup.contains("item continuation"));
        assert!(d.lines.iter().any(|l| matches!(l.style, Style::Text) && l.markup.contains("new paragraph")));
    }
}

#[cfg(test)]
mod footnotes_ii_tests {
    use super::*;

    #[test]
    fn multiline_footnote_continuations_are_collected() {
        let d = parse("Text[^note]\n\n[^note]: First line\n    second line with **bold**\n    third line");
        let foot: Vec<_> = d.lines.iter().filter(|l| matches!(l.style, Style::Footnote)).collect();
        assert!(foot.iter().any(|l| l.markup.contains("First line")));
        assert!(foot.iter().any(|l| l.markup.contains("second line with <b>bold</b>")));
        assert!(foot.iter().any(|l| l.markup.contains("third line")));
    }

    #[test]
    fn footnotes_support_links_lists_and_code() {
        let d = parse(
            "Ref[^n]\n\n[^n]: See [Rust](https://www.rust-lang.org/)\n    - list item\n    ```rust\n    let x = 1;\n    ```",
        );
        let foot: Vec<_> = d.lines.iter().filter(|l| matches!(l.style, Style::Footnote)).collect();
        assert!(foot.iter().any(|l| l.markup.contains("href=\"https://www.rust-lang.org/\"")));
        assert!(foot.iter().any(|l| l.markup.contains("•  list item")));
        assert!(foot.iter().any(|l| l.markup.contains("<tt>let x = 1;</tt>")));
    }

    #[test]
    fn repeated_footnote_references_get_backlinks() {
        let d = parse("One[^same] and two[^same].\n\n[^same]: Reused note.");
        let text = d.lines.iter().find(|l| matches!(l.style, Style::Text)).unwrap();
        assert!(text.extra_anchors.iter().any(|a| a == "fnref_same_1"));
        assert!(text.extra_anchors.iter().any(|a| a == "fnref_same_2"));
        let backs = d.lines.iter().find(|l| matches!(l.style, Style::Footnote) && l.markup.contains("Back:")).unwrap();
        assert!(backs.markup.contains("#fnref_same_1"));
        assert!(backs.markup.contains("#fnref_same_2"));
    }
}

#[cfg(test)]
mod commonmark_gfm_regression_v039_tests {
    use super::*;

    #[test]
    fn headings_cover_atx_and_setext_forms() {
        let d = parse("# H1\n\n###### H6\n\nSetext one\n==========\n\nSetext two\n----------");
        assert!(d.lines.iter().any(|l| matches!(l.style, Style::H1) && l.markup.contains("H1")));
        assert!(d.lines.iter().any(|l| matches!(l.style, Style::H6) && l.markup.contains("H6")));
        assert!(d.lines.iter().filter(|l| matches!(l.style, Style::H1)).count() >= 2);
        assert!(d.lines.iter().any(|l| matches!(l.style, Style::H2) && l.markup.contains("Setext two")));
    }

    #[test]
    fn gfm_tasks_and_strikethrough_survive_together() {
        let d = parse("- [x] ~~old~~ done\n- [ ] pending");
        assert!(matches!(d.lines[0].style, Style::Task(true)));
        assert!(d.lines[0].markup.contains("<s>old</s>"));
        assert!(matches!(d.lines[1].style, Style::Task(false)));
    }

    #[test]
    fn backtick_and_tilde_fences_are_distinct() {
        let d = parse("````rust\nlet x = 1;\n```\nstill code\n````\n\n~~~python\nprint('ok')\n~~~");
        let codes: Vec<_> = d.lines.iter().filter(|l| matches!(l.style, Style::Code)).collect();
        assert_eq!(codes.len(), 2);
        assert!(codes[0].code.as_ref().unwrap().text.contains("still code"));
        assert_eq!(codes[1].code.as_ref().unwrap().language, "python");
    }

    #[test]
    fn inline_code_can_contain_shorter_backtick_runs() {
        let d = parse("Use ``a ` b`` here.");
        assert!(d.lines[0].markup.contains("<tt>a ` b</tt>"));
    }

    #[test]
    fn reference_and_inline_links_are_both_collected() {
        let d = parse("[Rust][r] and [local](docs/a.md#part).\n\n[r]: https://www.rust-lang.org/ \"Rust\"");
        assert!(d.links.iter().any(|l| l.target == "https://www.rust-lang.org/"));
        assert!(d.links.iter().any(|l| l.target == "docs/a.md#part"));
    }

    #[test]
    fn gfm_table_alignment_is_retained() {
        let d = parse("| L | C | R |\n| :-- | :-: | --: |\n| a | b | c |");
        let row = d.lines.iter().find(|l| l.table.is_some()).unwrap();
        let table = row.table.as_ref().unwrap();
        assert_eq!(table.align, vec![TableAlign::Left, TableAlign::Center, TableAlign::Right]);
    }

    #[test]
    fn footnote_references_do_not_destroy_surrounding_text() {
        let d = parse("Before[^n] after.\n\n[^n]: Note text.");
        let line = d.lines.iter().find(|l| matches!(l.style, Style::Text)).unwrap();
        assert!(line.markup.contains("Before"));
        assert!(line.markup.contains("after"));
        assert!(d.lines.iter().any(|l| matches!(l.style, Style::Footnote)));
    }

    #[test]
    fn nested_quote_list_keeps_container_semantics() {
        let d = parse("> - outer\n>   - inner");
        let lists: Vec<_> = d.lines.iter().filter(|l| matches!(l.style, Style::Bullet)).collect();
        assert_eq!(lists.len(), 2);
        assert!(lists[1].indent >= lists[0].indent);
    }

    #[test]
    fn thematic_break_variants_are_recognized() {
        for src in ["---", "***", "___", "- - -", "* * *", "_ _ _"] {
            let d = parse(src);
            assert!(d.lines.iter().any(|l| matches!(l.style, Style::Rule)), "not a rule: {src}");
        }
    }

    #[test]
    fn github_math_fence_is_not_a_code_block() {
        let d = parse("```math\n\\frac{a}{b}\n```");
        assert!(d.lines.iter().any(|l| matches!(l.style, Style::Math)));
        assert!(!d.lines.iter().any(|l| matches!(l.style, Style::Code)));
    }

    #[test]
    fn raw_html_remains_inert_text() {
        let d = parse("<script>alert('x')</script>");
        assert!(d.lines.iter().any(|l| matches!(l.style, Style::Text)));
        assert!(d.lines.iter().any(|l| l.markup.contains("&lt;script&gt;")));
    }

    #[test]
    fn regression_suite_parses_without_losing_core_blocks() {
        let src = include_str!("../examples/regression-suite.md");
        let d = parse(src);
        assert!(d.lines.len() > 20);
        assert!(d.lines.iter().any(|l| matches!(l.style, Style::TableRow)));
        assert!(d.lines.iter().any(|l| matches!(l.style, Style::Code)));
        assert!(d.lines.iter().any(|l| matches!(l.style, Style::Math)));
        assert!(d.lines.iter().any(|l| matches!(l.style, Style::Footnote)));
        assert!(d.links.len() >= 4);
    }
    #[test]
    fn mermaid_flowchart_becomes_diagram_block() {
        let d = parse("```mermaid\nflowchart TD\nA[Start] --> B{Valid?}\n```");
        assert!(d.lines.iter().any(|l| matches!(l.style, Style::Diagram)));
        assert!(d.lines.iter().any(|l| l.diagram.is_some()));
    }

    #[test]
    fn unsupported_mermaid_falls_back_to_code() {
        let d = parse("```mermaid\nsequenceDiagram\nA->>B: hello\n```");
        assert!(!d.lines.iter().any(|l| matches!(l.style, Style::Diagram)));
        assert!(d.lines.iter().any(|l| matches!(l.style, Style::Code)));
    }

    #[test]
    fn graphviz_digraph_becomes_diagram_block() {
        let d = parse("```dot\ndigraph G { a -> b; }\n```");
        assert!(d.lines.iter().any(|l| matches!(l.style, Style::Diagram)));
        assert!(d.lines.iter().any(|l| l.diagram.is_some()));
    }

    #[test]
    fn unsupported_dot_falls_back_to_code() {
        let d = parse("```dot\nnot-a-dot-document\n```");
        assert!(!d.lines.iter().any(|l| matches!(l.style, Style::Diagram)));
        assert!(d.lines.iter().any(|l| matches!(l.style, Style::Code)));
    }

    #[test]
    fn malformed_documents_degrade_gracefully() {
        let samples = [
            "[broken link](https://example.org/(unterminated",
            "```rust\nfn main() {",
            "$$\\frac{1}{2}",
            "[^note]: first line\n    ```rust\n    unterminated",
            "<https://example.org/unterminated",
            "| a | b |\n| --- |\n| 1 | 2 | 3 |",
            "> > > - deeply nested\ncontinuation",
            "&not-a-real-entity; &#x110000;",
        ];
        for sample in samples {
            let document = parse(sample);
            assert!(!document.lines.is_empty(), "parser dropped malformed input: {sample:?}");
        }
    }

    #[test]
    fn large_synthetic_document_parses() {
        let mut source = String::with_capacity(512 * 1024);
        for i in 0..2_000 {
            source.push_str(&format!(
                "## Section {i}\n\nParagraph {i} with **bold**, `code`, and [link](https://example.org/{i}).\n\n- one\n  - two\n\n"
            ));
        }
        let document = parse(&source);
        assert!(document.lines.len() >= 6_000);
        assert!(document.links.len() >= 2_000);
    }
}

#[cfg(test)]
mod commonmark_completion_tests {
    use super::*;

    #[test]
    fn indented_code_block_is_detected() {
        let d = parse("    let x = 1;\n    println!(\"{x}\");");
        assert_eq!(d.lines.len(), 1);
        assert!(matches!(d.lines[0].style, Style::Code));
        let code = d.lines[0].code.as_ref().expect("code block");
        assert_eq!(code.text, "let x = 1;\nprintln!(\"{x}\");");
    }

    #[test]
    fn indented_code_does_not_interrupt_paragraph() {
        let d = parse("paragraph\n    continuation");
        assert_eq!(d.lines.len(), 1);
        assert!(matches!(d.lines[0].style, Style::Text));
        assert!(d.lines[0].markup.contains("continuation"));
    }

    #[test]
    fn ordered_list_parenthesis_marker_is_detected() {
        let d = parse("1) first\n12) twelfth");
        assert!(matches!(d.lines[0].style, Style::Ordered(1)));
        assert!(matches!(d.lines[1].style, Style::Ordered(12)));
    }

    #[test]
    fn ordered_list_accepts_up_to_nine_digits() {
        let d = parse("123456789) item\n1234567890) not-an-item");
        assert!(matches!(d.lines[0].style, Style::Ordered(123456789)));
        assert!(matches!(d.lines[1].style, Style::Text));
    }

    #[test]
    fn email_autolink_is_detected() {
        let d = parse("Contact <user@example.com>.");
        assert_eq!(d.links.len(), 1);
        assert_eq!(d.links[0].target, "mailto:user@example.com");
        assert!(d.lines[0].markup.contains("user@example.com"));
    }
}

#[cfg(test)]
mod commonmark_v1_release_tests {
    use super::*;

    #[test]
    fn underscore_emphasis_and_strong_are_supported() {
        let d = parse("_emphasis_ and __strong__ and word_with_underscore.");
        assert!(d.lines[0].markup.contains("<i>emphasis</i>"));
        assert!(d.lines[0].markup.contains("<b>strong</b>"));
        assert!(d.lines[0].markup.contains("word_with_underscore"));
    }

    #[test]
    fn all_ascii_punctuation_can_be_backslash_escaped() {
        let d = parse(r#"\!\"\#\$\%\&\'\(\)\*\+\,\-\.\/\:\;\<\=\>\?\@\[\\\]\^\_\`\{\|\}\~"#);
        assert!(matches!(d.lines[0].style, Style::Text));
        assert!(!d.lines[0].markup.contains("\\!"));
        assert!(d.lines[0].markup.contains("&lt;"));
        assert!(d.lines[0].markup.contains("&gt;"));
    }

    #[test]
    fn named_entities_include_commonmark_html5_set_examples() {
        let d = parse("&AElig; &NotEqualTilde; &CounterClockwiseContourIntegral;");
        let m = &d.lines[0].markup;
        assert!(m.contains('Æ'));
        assert!(m.contains('∳'));
        assert!(!m.contains("&AElig;"));
    }

    #[test]
    fn fenced_code_allows_at_most_three_leading_spaces() {
        for n in 0..=3 {
            let src = format!("{}```rust\nlet x = 1;\n{}```", " ".repeat(n), " ".repeat(n));
            let d = parse(&src);
            assert!(d.lines.iter().any(|l| matches!(l.style, Style::Code)), "indent {n}");
        }
        let d = parse("    ```rust\n    let x = 1;\n    ```");
        assert!(d.lines.iter().all(|l| l.code.as_ref().is_none_or(|c| c.language.is_empty())));
    }

    #[test]
    fn backslash_hard_break_joins_lines_with_a_break() {
        let d = parse("one\\\ntwo\\\nthree");
        assert_eq!(d.lines.len(), 1);
        assert_eq!(d.lines[0].markup, "one\ntwo\nthree");
        // An escaped backslash is literal, and a trailing one ends the paragraph.
        assert!(!parse("one\\\\\ntwo").lines[0].markup.contains('\n'));
        assert_eq!(parse("end\\").lines[0].markup, "end\\");
    }

    #[test]
    fn deeply_indented_list_items_stay_nested_lists() {
        let d = parse("- a\n  - b\n    - c\n    - d\n  - e");
        assert!(d.lines.iter().all(|l| matches!(l.style, Style::Bullet)));
        assert_eq!(d.lines.iter().map(|l| l.indent).collect::<Vec<_>>(), [0, 1, 2, 2, 1]);
        let ordered = parse("1. a\n   1. b\n      1. c");
        assert!(ordered.lines.iter().all(|l| matches!(l.style, Style::Ordered(_))));
        // A real indented code block after a blank line is still code.
        assert!(parse("- a\n\n        code").lines.iter().any(|l| matches!(l.style, Style::Code)));
    }

    #[test]
    fn blocks_group_tables_and_skip_unrenderable_lines() {
        let d = parse("intro\n\n| a | b |\n| --- | --- |\n| 1 | 2 |\n| 3 | 4 |\n\noutro");
        let kinds: Vec<_> = d.index.blocks.iter().map(|b| matches!(b, Block::Table { .. })).collect();
        assert_eq!(kinds.iter().filter(|t| **t).count(), 1);
        let Some(Block::Table { first, end }) = d.index.blocks.iter().copied().find(|b| matches!(b, Block::Table { .. })) else {
            panic!("no table block");
        };
        assert_eq!(end - first, 4, "header, separator and two body rows");
        assert_eq!(d.index.blocks.len(), d.index.search_text.len());
    }

    #[test]
    fn block_anchors_point_at_the_block_holding_them() {
        let d = parse("# Top\n\ntext\n\n## Second part\n\nmore");
        let at = d.index.anchors["second_part"];
        let Block::Line(i) = d.index.blocks[at] else { panic!("heading is a line block") };
        assert!(matches!(d.lines[i].style, Style::H2));
        assert_eq!(d.index.anchors["top"], 0);
    }

    #[test]
    fn search_hits_count_table_cells_separately() {
        let d = parse("Hello World\n\n| Hello | x |\n| --- | --- |\n| hello | Hello |");
        let hits: Vec<usize> = d
            .index
            .blocks
            .iter()
            .zip(&d.index.search_text)
            .map(|(b, t)| count_hits(t, "hello", matches!(b, Block::Table { .. })))
            .collect();
        // The blank line between the paragraph and the table is a block of its own.
        assert_eq!(hits, [1, 0, 3]);
        assert_eq!(count_hits("anything", "", false), 0);
    }

    #[test]
    fn display_url_decodes_only_text_not_syntax() {
        assert_eq!(display_url("file:///a/Olivelle%20%E2%80%94%20Early.md"), "file:///a/Olivelle — Early.md");
        assert_eq!(display_url("https://x.org/a%2Fb%3Fc%23d"), "https://x.org/a%2Fb%3Fc%23d");
        assert_eq!(display_url("https://x.org/p.md#els%C5%91_l%C3%A9p"), "https://x.org/p.md#első_lép");
        // An invalid UTF-8 run is left alone, as is a truncated escape.
        assert_eq!(display_url("a%E2%80b"), "a%E2%80b");
        assert_eq!(display_url("100%"), "100%");
    }
}
