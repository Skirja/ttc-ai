//! Conservative, streaming semantic filters. Unknown text is never dropped.
use crate::command::{Channel, OutputEvent};
pub mod diagnostics;
pub mod rules;
pub mod structured;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct FilterPlan {
    pub families: Vec<String>,
}
#[derive(Default, Debug, Clone, Serialize)]
pub struct FilterStats {
    pub rewritten_lines: u64,
    pub duplicate_diagnostics: u64,
    pub suppressed_lines: u64,
    pub passed_items: u64,
    pub progress_lines: u64,
}
pub struct Filter {
    renderer: structured::Renderer,
    diagnostics: diagnostics::Deduplicator,
    plan: FilterPlan,
    pending: [Vec<u8>; 2],
    stats: FilterStats,
}
impl Filter {
    pub fn new(families: Vec<String>) -> Self {
        Self {
            renderer: structured::Renderer::default(),
            diagnostics: diagnostics::Deduplicator::default(),
            plan: FilterPlan { families },
            pending: [vec![], vec![]],
            stats: FilterStats::default(),
        }
    }
    fn line(&mut self, channel: Channel, bytes: Vec<u8>) -> Option<OutputEvent> {
        let Ok(s) = std::str::from_utf8(&bytes) else {
            return Some(OutputEvent { channel, bytes });
        };
        // Do not normalize or strip unrecognized lines; preserve source and diagnostics exactly.
        let s = s.trim_end_matches(['\r', '\n']);
        if self.diagnostics.duplicate(
            &self.plan.families,
            if channel == Channel::Stdout { 0 } else { 1 },
            s,
        ) {
            self.stats.duplicate_diagnostics += 1;
            return None;
        }
        if let Some(rendered) = self.renderer.line(
            &self.plan.families,
            if channel == Channel::Stdout { 0 } else { 1 },
            s,
        ) {
            self.stats.rewritten_lines += 1;
            return Some(OutputEvent {
                channel,
                bytes: format!("{rendered}\n").into_bytes(),
            });
        }
        if let Some(kind) = rules::match_line(&self.plan.families, s) {
            self.stats.suppressed_lines += 1;
            match kind {
                rules::Kind::Passing => self.stats.passed_items += 1,
                rules::Kind::Progress => self.stats.progress_lines += 1,
            }
            return None;
        }
        Some(OutputEvent { channel, bytes })
    }
    pub fn feed(&mut self, event: OutputEvent) -> Vec<OutputEvent> {
        let i = if event.channel == Channel::Stdout {
            0
        } else {
            1
        };
        self.pending[i].extend(event.bytes);
        let mut out = vec![];
        while let Some(pos) = self.pending[i].iter().position(|b| *b == b'\n') {
            let line = self.pending[i].drain(..=pos).collect();
            if let Some(e) = self.line(event.channel, line) {
                out.push(e);
            }
        }
        // A megabyte-long line is raw, with bounded memory and no truncation.
        if self.pending[i].len() > 1024 * 1024 {
            out.push(OutputEvent {
                channel: event.channel,
                bytes: std::mem::take(&mut self.pending[i]),
            });
        }
        out
    }
    pub fn finish(&mut self) -> Vec<OutputEvent> {
        let mut out = vec![];
        for i in 0..2 {
            if !self.pending[i].is_empty() {
                out.push(OutputEvent {
                    channel: if i == 0 {
                        Channel::Stdout
                    } else {
                        Channel::Stderr
                    },
                    bytes: std::mem::take(&mut self.pending[i]),
                });
            }
        }
        if self.stats.duplicate_diagnostics > 0 {
            out.push(OutputEvent{channel:Channel::Stderr,bytes:format!("\nTTC: {} identical location-bearing diagnostics repeated (first occurrence retained)\n",self.stats.duplicate_diagnostics).into_bytes()});
        }
        if self.stats.suppressed_lines > 0 {
            out.push(OutputEvent {
                channel: Channel::Stderr,
                bytes: format!(
                    "\nTTC: {} passing records, {} progress lines compacted\n",
                    self.stats.passed_items, self.stats.progress_lines
                )
                .into_bytes(),
            });
        }
        out
    }
    pub fn stats(&self) -> &FilterStats {
        &self.stats
    }
}
