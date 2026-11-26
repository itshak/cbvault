//! Fingerprint complete native annotation payloads, moves and keys.
#![allow(missing_docs)]
use cbvault::bridge::{Database, GameRef, GameSink, convert_parallel};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::time::Instant;

#[derive(Default)]
struct Sink {
    digest: DefaultHasher,
    games: u64,
    annotated: u64,
    bytes: u64,
    plies: u64,
}
impl GameSink for Sink {
    fn wants_annotations(&self) -> bool {
        true
    }
    fn wants_keys(&self) -> bool {
        true
    }
    fn game(&mut self, g: GameRef<'_>) {
        self.games += 1;
        assert_eq!(self.games, u64::from(g.id));
        self.plies += g.moves.len() as u64;
        assert_eq!(g.keys.len(), g.moves.len() + 1);
        g.id.hash(&mut self.digest);
        g.header.bytes().hash(&mut self.digest);
        g.moves.hash(&mut self.digest);
        g.keys.hash(&mut self.digest);
        if let Some(a) = g.twocbh_annotations {
            self.annotated += 1;
            self.bytes += a.content().len() as u64;
            a.content().hash(&mut self.digest);
        }
    }
    fn failed(&mut self, id: u32, error: &cbvault::bridge::Error) {
        panic!("failed {id}: {error}");
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<_> = std::env::args().collect();
    let db = Database::open(&a[1])?;
    let threads = a[2].parse()?;
    let mut sink = Sink::default();
    let start = Instant::now();
    let stats = convert_parallel(&db, &mut sink, threads, 0)?;
    let secs = start.elapsed().as_secs_f64();
    assert!(stats.complete && stats.failures == 0);
    assert_eq!(stats.games, sink.games);
    assert_eq!(stats.plies, sink.plies);
    println!(
        "{{\"seconds\":{secs:.9},\"games\":{},\"plies\":{},\"annotated\":{},\"bytes\":{},\"fingerprint\":\"{:016x}\"}}",
        sink.games,
        sink.plies,
        sink.annotated,
        sink.bytes,
        sink.digest.finish()
    );
    Ok(())
}
