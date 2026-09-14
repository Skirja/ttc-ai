use ttc_ai::{
    command::{Channel, OutputEvent},
    filtering::Filter,
};
fn render(family: &str, s: &str) -> String {
    let mut f = Filter::new(vec![family.into()]);
    let mut out = vec![];
    for e in f.feed(OutputEvent {
        channel: Channel::Stdout,
        bytes: s.as_bytes().to_vec(),
    }) {
        out.extend(e.bytes);
    }
    for e in f.finish() {
        out.extend(e.bytes);
    }
    String::from_utf8(out).unwrap()
}
#[test]
fn git_status_preserves_states_names_and_conflicts() {
    let s = "On branch feature\nChanges to be committed:\n\tmodified:   src/a.rs\nChanges not staged for commit:\n\tdeleted:    src/b.rs\nUntracked files:\n\tnew file with spaces\nUnmerged paths:\n\tboth modified:   src/c.rs\n";
    let out = render("git-status", s);
    assert_eq!(
        out,
        "branch feature\nstaged:\n  M src/a.rs\nunstaged:\n  D src/b.rs\nuntracked:\n\tnew file with spaces\nunmerged:\n  UU src/c.rs\n"
    );
}
#[test]
fn search_preserves_every_match_and_line() {
    let s = "src/a.rs:12:fn main() {\nsrc/a.rs:15:  panic!(\"diagnostic\");\nsrc/b.rs:8:fn other() {}\n";
    assert_eq!(
        render("search-lines", s),
        "src/a.rs:\n  12: fn main() {\n  15:   panic!(\"diagnostic\");\nsrc/b.rs:\n  8: fn other() {}\n"
    );
}
#[test]
fn unknown_search_and_git_records_unchanged() {
    for f in ["git-status", "search-lines"] {
        let s = "fatal: future diagnostic\nwarning: preserve\nUnicode αβ no newline";
        assert_eq!(render(f, s), s);
    }
}
#[test]
fn duplicate_diagnostics_have_explicit_counts() {
    let s = "src/a.ts(3,7): error TS2322: expected number\nsrc/a.ts(3,7): error TS2322: expected number\nsrc/b.ts(3,7): error TS2322: expected number\n";
    let out = render("typescript", s);
    assert_eq!(out.matches("src/a.ts(3,7)").count(), 1);
    assert!(out.contains("src/b.ts(3,7)"));
    assert!(out.contains("1 identical location-bearing diagnostics repeated"));
}
#[test]
fn rust_context_only_warnings_are_not_deduplicated() {
    let s = "warning: unused variable\n  --> src/a.rs:2:3\nwarning: unused variable\n  --> src/b.rs:2:3\n";
    assert_eq!(render("rust", s), s);
}
#[test]
fn diagnostic_fields_preserve_position_code_and_message() {
    let d = ttc_ai::filtering::diagnostics::parse("src/a.ts(3,7): error TS2322: expected number")
        .unwrap();
    assert_eq!(d.file, "src/a.ts");
    assert_eq!(d.line, 3);
    assert_eq!(d.column, Some(7));
    assert_eq!(d.code.as_deref(), Some("TS2322"));
    assert_eq!(d.message, "expected number");
}
