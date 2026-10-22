use crate::{buffer::Buffer, defs::*, display::*, editor::Editor, terminal::Terminal};
fn editor(text: &str) -> Editor {
    Editor::new(
        Config::default(),
        Terminal {
            out: std::io::stdout(),
            rows: 24,
            cols: 80,
            mouse: false,
            suspended: false,
            active: false,
            preserve: false,
        },
        vec![Buffer::from_string(text, true)],
    )
}
#[test]
fn formats_round_trip() {
    for text in ["", "a\n", "a\nb", "a\r\nb\r\n", "a\rb\r", "世界\0\n"] {
        assert_eq!(Buffer::from_string(text, true).serialize(), text);
    }
}
#[test]
fn no_conversion_never_adds_carriage_returns() {
    let text = "a\r\nb\r\n";
    let b = Buffer::from_string(text, false);
    assert_eq!(b.fmt, Format::Nix);
    assert_eq!(b.serialize(), text);
}
#[test]
fn display_columns_include_controls_and_wide_chars() {
    let chars = "a\t界\x01e\u{301}".chars().collect::<Vec<_>>();
    assert_eq!(column_of(&chars, chars.len(), 8), 13);
    assert_eq!(actual_x(&chars, 1, 8), 1);
    assert_eq!(actual_x(&chars, 7, 8), 1);
    assert_eq!(actual_x(&chars, 8, 8), 2);
    assert_eq!(actual_x(&chars, 9, 8), 2);
    assert_eq!(actual_x(&chars, 10, 8), 3);
}
#[test]
fn new_branch_preserves_prior_undo_and_saved_identity() {
    let mut ed = editor("");
    ed.inject_text(&['a']);
    ed.last_action = UndoType::Other;
    ed.inject_text(&['b']);
    ed.b().set_saved();
    ed.b().undo();
    ed.last_action = UndoType::Other;
    ed.inject_text(&['c']);
    assert_eq!(ed.borig().serialize(), "ac\n");
    assert!(ed.borig().is_modified());
    ed.b().undo();
    assert_eq!(ed.borig().serialize(), "a\n");
    ed.b().undo();
    assert_eq!(ed.borig().serialize(), "");
}
#[test]
fn save_breaks_typing_group() {
    let mut ed = editor("");
    ed.inject_text(&['a']);
    ed.b().set_saved();
    ed.inject_text(&['b']);
    ed.b().undo();
    assert_eq!(ed.borig().serialize(), "a\n");
    assert!(!ed.borig().is_modified());
}
#[test]
fn replacement_restores_mark_and_contents_on_undo() {
    let mut ed = editor("αβ\nγδ\n");
    ed.b().mark = Some(Pos::new(0, 1));
    ed.goto_pos(1, 1);
    let original = ed.borig().serialize();
    ed.replace_range(Pos::new(0, 1), Pos::new(1, 1), "X\nY\nZ", UndoType::Replace);
    assert_eq!(ed.borig().serialize(), "αX\nY\nZδ\n");
    ed.b().undo();
    assert_eq!(ed.borig().serialize(), original);
    assert_eq!(ed.borig().mark, Some(Pos::new(0, 1)));
    ed.b().redo();
    assert_eq!(ed.borig().serialize(), "αX\nY\nZδ\n");
}
#[test]
fn regex_search_counts_unicode_characters() {
    let b = Buffer::from_string("α世 α世\n", true);
    let re = crate::search::expression("(α)(世)", true, true).unwrap();
    let m = crate::search::matches(&b.lines, &re, Some(r"\2\1"));
    assert_eq!(m.len(), 2);
    assert_eq!(m[1].pos, Pos::new(0, 3));
    assert_eq!(m[0].replacement, "世α");
}
#[test]
fn multiline_syntax_retains_state() {
    let b = Buffer::from_string("/* a\nb */ let\n", true);
    let s = crate::syntax::Syntax {
        name: "test".into(),
        rules: vec![crate::syntax::Rule {
            color: "34".into(),
            start: crate::syntax::compile(r"/\*", false).unwrap(),
            end: Some(crate::syntax::compile(r"\*/", false).unwrap()),
        }],
        ..Default::default()
    };
    let colors = s.highlight(&b.lines, 3);
    assert_eq!(colors[0][0], Some("34"));
    assert_eq!(colors[1][0], Some("34"));
    assert_eq!(colors[1][4], None);
}
#[test]
fn edit_history_round_trips_random_operations() {
    for seed in 1..=12u64 {
        let mut rng = seed;
        let mut ed = editor("start\n世界\n");
        let original = ed.borig().serialize();
        for _ in 0..150 {
            rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
            let r = (rng >> 32) as usize;
            let y = r % ed.borig().lines.len();
            let x = r / 13 % (ed.borig().line_len(y) + 1);
            ed.goto_pos(y, x);
            ed.last_action = UndoType::Other;
            ed.keep_cutbuffer = false;
            match r % 8 {
                0 => ed.inject_text(&['x', '界']),
                1 => ed.enter_key(),
                2 => ed.delete_forward(),
                3 => ed.backspace(),
                4 => ed.cut_text(),
                5 => ed.paste_text(),
                6 => ed.do_indent(),
                _ => ed.do_unindent(),
            }
            assert!(!ed.borig().lines.is_empty());
            assert!(ed.borig().cursor.y < ed.borig().lines.len());
            assert!(ed.borig().cursor.x <= ed.borig().line_len(ed.borig().cursor.y));
        }
        let final_text = ed.borig().serialize();
        while ed.b().undo().is_some() {}
        assert_eq!(ed.borig().serialize(), original, "seed {seed}");
        while ed.b().redo().is_some() {}
        assert_eq!(ed.borig().serialize(), final_text, "seed {seed}");
    }
}
#[test]
fn wrap_chunks_cover_width_and_cursor_at_edge() {
    let chars = "12345678".chars().collect::<Vec<_>>();
    let chunks = compute_chunks(&chars, 4, 8, false);
    assert_eq!(
        chunks.iter().map(|c| c.left).collect::<Vec<_>>(),
        vec![0, 4, 8]
    );
}
#[test]
fn wraps_always_make_progress() {
    for text in [
        "",
        "\t\t\t",
        "世界世界世界",
        "a\u{301}b\u{301}c\u{301}",
        "a b c d e",
    ] {
        for cols in 1..20 {
            let chars = text.chars().collect::<Vec<_>>();
            let chunks = compute_chunks(&chars, cols, 8, true);
            assert!(!chunks.is_empty());
            for pair in chunks.windows(2) {
                assert!(pair[1].left > pair[0].left);
                assert_eq!(pair[0].right, pair[1].left);
            }
            assert!(chunks.len() < 100);
        }
    }
}

#[test]
fn control_arrows_and_raw_control_aliases_resolve() {
    use crate::keys::{Key, F};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let ed = editor("");
    assert_eq!(
        ed.binding(MMAIN, Key::Right(true, false)),
        Some(F::NextWord)
    );
    assert_eq!(ed.binding(MMAIN, Key::Left(true, false)), Some(F::PrevWord));
    for (raw, expected) in [('4', F::Replace), ('5', F::Complete), ('7', F::GoToLine)] {
        let key =
            crate::terminal::convert_key(&KeyEvent::new(KeyCode::Char(raw), KeyModifiers::CONTROL))
                .unwrap();
        assert_eq!(ed.binding(MMAIN, key), Some(expected));
    }
}

#[test]
fn prompt_clipping_does_not_split_utf8_or_execute_controls() {
    assert_eq!(crate::render::slice_columns("ab世界cd", 3, 4, 8), " 界c");
    assert_eq!(crate::render::slice_columns("x\x1b[31m", 0, 4, 8), "x^[[");
}

#[test]
fn completion_cycles_and_remains_undoable() {
    let mut ed = editor("alpha alpine\nal\n");
    ed.goto_pos(1, 2);
    ed.complete_word();
    assert_eq!(ed.borig().line(1).iter().collect::<String>(), "alpha");
    ed.complete_word();
    assert_eq!(ed.borig().line(1).iter().collect::<String>(), "alpine");
    ed.b().undo();
    assert_eq!(ed.borig().line(1).iter().collect::<String>(), "alpha");
    ed.b().undo();
    assert_eq!(ed.borig().line(1).iter().collect::<String>(), "al");
}

#[test]
fn posix_regex_uses_longest_alternative_and_backreferences() {
    let b = Buffer::from_string("aa aa", true);
    let re = crate::search::expression("(a|aa)", true, true).unwrap();
    let m = crate::search::matches(&b.lines, &re, Some(r"[\1]"));
    assert_eq!(m[0].len, 2);
    assert_eq!(m[0].replacement, "[aa]");
    let re = crate::search::expression(r"(aa) \1", true, true).unwrap();
    assert_eq!(crate::search::matches(&b.lines, &re, None).len(), 1);
}

#[test]
fn justify_keeps_paragraphs_separate_and_undo_restores_them() {
    let mut ed = editor("first paragraph has several words\n\nsecond paragraph stays separate\n");
    ed.cfg.fill = 15;
    ed.cfg.trimblanks = true;
    let original = ed.borig().serialize();
    ed.justify(true);
    assert!(ed.borig().serialize().contains("\n\n"));
    for line in &ed.borig().lines {
        assert!(wideness(&line.chars, 8) <= 15);
    }
    ed.b().undo();
    assert_eq!(ed.borig().serialize(), original);
}

#[test]
fn nanorc_hex_colors_and_posix_quoted_strings_are_accepted() {
    let words = crate::config::words(r##"icolor #0a4 "\<(int|text)\>""##);
    assert_eq!(words, vec!["icolor", "#0a4", r"\<(int|text)\>"]);
    let rule = crate::syntax::compile(r#""([^"\]|\\.)*"|<[^= \t]*>"#, false).unwrap();
    assert!(rule.is_match(r#".ascii "hello""#));
    assert_eq!(crate::syntax::color("#0a4"), "38;2;0;170;68");
}
