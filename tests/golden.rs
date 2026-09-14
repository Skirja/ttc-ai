use serde::Deserialize;
use ttc_ai::{
    command::{Channel, OutputEvent},
    filtering::Filter,
};
#[derive(Deserialize)]
struct Fixture {
    family: String,
    raw: String,
    expected: String,
}
#[test]
fn semantic_golden_fixtures() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/semantic");
    let mut count = 0;
    for entry in std::fs::read_dir(root).unwrap() {
        let p = entry.unwrap().path();
        let fixture: Fixture = serde_json::from_slice(&std::fs::read(&p).unwrap()).unwrap();
        for chunk_size in [1, 17, 16384] {
            let mut f = Filter::new(vec![fixture.family.clone()]);
            let mut out = vec![];
            for chunk in fixture.raw.as_bytes().chunks(chunk_size) {
                for e in f.feed(OutputEvent {
                    channel: Channel::Stdout,
                    bytes: chunk.to_vec(),
                }) {
                    out.extend(e.bytes);
                }
            }
            for e in f.finish() {
                out.extend(e.bytes);
            }
            assert_eq!(
                String::from_utf8(out).unwrap(),
                fixture.expected,
                "{} chunk={chunk_size}",
                p.display()
            );
        }
        count += 1;
    }
    assert!(count >= 100);
}
