// Copyright (c) 2026 the oschess-cb-bridge contributors
// Modified for cbvault (MIT License). See docs/provenance.md.

//! `.2cba` records: the annotations of one game.
//!
//! A record is a run of position blocks, each starting with `position: i32`
//! (−1 for the game as a whole, or the ply index) and `count: i32`, followed by
//! `count` annotations, terminated by the marker `0x7fffffff`.
//!
//! Each annotation starts with a 2-byte type code (`u16 LE`). Unknown types
//! stop the decoding cleanly and report the unsupported type code via
//! [`Annotations::stopped_at`].

use std::path::Path;

use super::record::Record;
use crate::cbh::bytes::text;
use crate::error::{Error, Result};
use crate::game::annotations::{Annotation, GAME_POSITION};

/// The tag of an annotation record's frame.
pub const ANNOTATION_TAG: u16 = 0x2000;
/// The end marker of a record's position blocks.
pub const END_MARKER: i32 = 0x7fff_ffff;

/// One item of an annotation record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Item<'a> {
    /// The move the annotation belongs to, or -1 for the game as a whole.
    pub position: i32,
    /// Byte offset within the content slice.
    pub offset: u32,
    /// The annotation itself.
    pub annotation: Annotation<'a>,
}

/// One game's `.2cba` annotation record.
#[derive(Clone, Copy, Debug)]
pub struct Annotations<'a> {
    record: Record<'a>,
}

impl<'a> Annotations<'a> {
    /// Splits a whole `.2cba` record of `path`, its magic and trailer included.
    #[inline]
    pub fn parse(path: &Path, raw: &'a [u8]) -> Result<Self> {
        Ok(Annotations { record: Record::parse(path, 0, raw)? })
    }

    /// The record's framing.
    #[inline]
    pub fn record(&self) -> &Record<'a> {
        &self.record
    }

    /// The record's content, exactly as stored.
    #[inline]
    pub fn content(&self) -> &'a [u8] {
        self.record.content()
    }

    /// Iterates over position blocks and their annotations.
    pub fn iter_items(&self) -> Items<'a> {
        Items {
            content: self.content(),
            offset: 0,
            curr_pos: -1,
            remaining_in_block: 0,
            stopped: false,
            stopped_at: None,
        }
    }

    /// Checks that all annotations are within the game's plies.
    pub fn check_positions(&self, moves: u32) -> Result<u32> {
        let mut past = 0u32;
        for item in self.iter_items() {
            let item = item?;
            if item.position < GAME_POSITION {
                return Err(Error::corrupt("<.2cba>", item.offset as u64, "negative position below -1"));
            }
            if item.position >= moves as i32 {
                past += 1;
            }
        }
        Ok(past)
    }

    /// The annotation text this record carries, as one string per run of
    /// printable bytes or text annotations.
    pub fn texts(&self) -> Vec<String> {
        let mut out = Vec::new();
        // First try decoded text items
        for item in self.iter_items().flatten() {
            if let Annotation::Text { text: t, .. } = item.annotation
                && !t.is_empty()
            {
                out.push(text(t));
            }
        }
        if !out.is_empty() {
            return out;
        }

        // Fall back to scanning printable runs in content
        let bytes = self.content();
        let mut start = 0;
        while start < bytes.len() {
            let rel = bytes[start..].iter().position(|&b| !(0x20..0x7f).contains(&b));
            match rel {
                Some(rel) => {
                    if rel >= MIN_TEXT {
                        out.push(text(&bytes[start..start + rel]));
                    }
                    start += rel + 1;
                }
                None => {
                    if bytes.len() - start >= MIN_TEXT {
                        out.push(text(&bytes[start..]));
                    }
                    break;
                }
            }
        }
        out
    }
}

const MIN_TEXT: usize = 4;

/// Iterator over items in `.2cba` content.
pub struct Items<'a> {
    content: &'a [u8],
    offset: usize,
    curr_pos: i32,
    remaining_in_block: usize,
    stopped: bool,
    /// When an unknown type code stops parsing: `Some((position, type_code))`.
    pub stopped_at: Option<(i32, u16)>,
}

impl<'a> Iterator for Items<'a> {
    type Item = Result<Item<'a>>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.stopped || self.offset >= self.content.len() {
            return None;
        }

        while self.remaining_in_block == 0 {
            if self.offset + 4 > self.content.len() {
                return None;
            }
            let pos = i32::from_le_bytes(self.content[self.offset..self.offset + 4].try_into().unwrap());
            self.offset += 4;
            if pos == END_MARKER {
                self.stopped = true;
                return None;
            }
            if pos < GAME_POSITION {
                self.stopped = true;
                return Some(Err(Error::corrupt("<.2cba>", self.offset as u64, "invalid position in block")));
            }
            if self.offset + 4 > self.content.len() {
                self.stopped = true;
                return Some(Err(Error::corrupt("<.2cba>", self.offset as u64, "truncated annotation count")));
            }
            let count = i32::from_le_bytes(self.content[self.offset..self.offset + 4].try_into().unwrap());
            self.offset += 4;
            if count < 0 {
                self.stopped = true;
                return Some(Err(Error::corrupt("<.2cba>", self.offset as u64, "negative annotation count")));
            }
            self.curr_pos = pos;
            self.remaining_in_block = count as usize;
        }

        self.remaining_in_block -= 1;
        let item_offset = self.offset as u32;

        if self.offset + 2 > self.content.len() {
            self.stopped = true;
            return Some(Err(Error::corrupt("<.2cba>", self.offset as u64, "truncated annotation type")));
        }
        let type_code = u16::from_le_bytes(self.content[self.offset..self.offset + 2].try_into().unwrap());
        self.offset += 2;

        match parse_annotation(self.content, &mut self.offset, type_code) {
            Ok(Some(annotation)) => Some(Ok(Item { position: self.curr_pos, offset: item_offset, annotation })),
            Ok(None) => {
                // Unknown type: stop cleanly
                self.stopped = true;
                self.stopped_at = Some((self.curr_pos, type_code));
                None
            }
            Err(e) => {
                self.stopped = true;
                Some(Err(e))
            }
        }
    }
}

struct ContentReader<'a> {
    content: &'a [u8],
    offset: usize,
}

impl<'a> ContentReader<'a> {
    fn bad(&self, msg: &str) -> Error {
        Error::corrupt("<.2cba>", self.offset as u64, msg)
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        if self.offset + n > self.content.len() {
            return Err(self.bad("annotation runs past content"));
        }
        let s = &self.content[self.offset..self.offset + n];
        self.offset += n;
        Ok(s)
    }

    fn i32_val(&mut self) -> Result<i32> {
        let b = self.take(4)?;
        Ok(i32::from_le_bytes(b.try_into().unwrap()))
    }

    fn u16_val(&mut self) -> Result<u16> {
        let b = self.take(2)?;
        Ok(u16::from_le_bytes(b.try_into().unwrap()))
    }
}

fn parse_annotation<'a>(content: &'a [u8], offset: &mut usize, t: u16) -> Result<Option<Annotation<'a>>> {
    let mut r = ContentReader { content, offset: *offset };
    let res = match t {
        0x02 | 0x82 => {
            r.take(2)?; // skip 2 zeros
            let language = r.u16_val()?;
            let n = r.i32_val()? as usize;
            let text_bytes = r.take(n)?;
            Ok(Some(Annotation::Text { before: t == 0x82, language, text: text_bytes }))
        }
        0x03 => {
            let b = r.take(3)?;
            Ok(Some(Annotation::Symbols { on_move: b[0], on_position: b[1], prefix: b[2] }))
        }
        0x04 => {
            let n = r.i32_val()? as usize;
            let items = r.take(n)?;
            if !items.len().is_multiple_of(2) {
                return Err(r.bad("colored squares length not even"));
            }
            for chunk in items.as_chunks::<2>().0 {
                if chunk[1] == 0 || chunk[1] > 64 {
                    return Err(r.bad("square out of 1..=64 range"));
                }
            }
            Ok(Some(Annotation::Squares(items)))
        }
        0x05 => {
            let n = r.i32_val()? as usize;
            let items = r.take(n)?;
            if !items.len().is_multiple_of(3) {
                return Err(r.bad("arrows length not multiple of 3"));
            }
            for chunk in items.as_chunks::<3>().0 {
                if chunk[1] == 0 || chunk[1] > 64 || chunk[2] == 0 || chunk[2] > 64 {
                    return Err(r.bad("arrow square out of 1..=64 range"));
                }
            }
            Ok(Some(Annotation::Arrows(items)))
        }
        // Other known fixed/variable payloads
        0x07 | 0x08 | 0x16 | 0x17 | 0x22 | 0x23 | 0x25 => {
            let data = r.take(4)?;
            Ok(Some(Annotation::Other { code: t, data }))
        }
        0x27 => {
            let data = r.take(2)?;
            Ok(Some(Annotation::Other { code: t, data }))
        }
        0x14 | 0x18 => {
            let data = r.take(1)?;
            Ok(Some(Annotation::Other { code: t, data }))
        }
        0x21 => {
            let data = r.take(6)?;
            Ok(Some(Annotation::Other { code: t, data }))
        }
        0x24 => {
            let data = r.take(38)?;
            Ok(Some(Annotation::Other { code: t, data }))
        }
        0x15 => {
            let n = r.i32_val()? as usize;
            let data = r.take(n)?;
            Ok(Some(Annotation::Other { code: t, data }))
        }
        0x20 => {
            let prefix = r.take(4)?;
            let n = r.i32_val()? as usize;
            let _body = r.take(n)?;
            let start = prefix.as_ptr() as usize - content.as_ptr() as usize;
            let len = 4 + 4 + n;
            Ok(Some(Annotation::Other { code: t, data: &content[start..start + len] }))
        }
        // Unknown type
        _ => Ok(None),
    };
    *offset = r.offset;
    res
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::twocbh::testdata::record_with_tag;

    #[test]
    fn parse_annotation_blocks() {
        let mut body = Vec::new();
        // Block 1: position 0 (first move), 2 annotations
        body.extend(0i32.to_le_bytes()); // position
        body.extend(2i32.to_le_bytes()); // count

        // Annotation 1: Text after move
        body.extend(0x02u16.to_le_bytes()); // type
        body.extend([0u8, 0u8]); // 2 zero bytes
        body.extend(1u16.to_le_bytes()); // language English
        let comment = b"A strong novelty!";
        body.extend((comment.len() as i32).to_le_bytes());
        body.extend_from_slice(comment);

        // Annotation 2: Symbols
        body.extend(0x03u16.to_le_bytes());
        body.extend([1u8, 2u8, 0u8]); // ! on move, += on pos

        // End marker
        body.extend(END_MARKER.to_le_bytes());

        let raw = record_with_tag(ANNOTATION_TAG, body.len() as u32, 0, &body);
        let ann = Annotations::parse(Path::new("test.2cba"), &raw).expect("parse annotations");

        let items: Vec<_> = ann.iter_items().collect::<Result<_>>().unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].position, 0);
        match items[0].annotation {
            Annotation::Text { before, language, text } => {
                assert!(!before);
                assert_eq!(language, 1);
                assert_eq!(text, comment);
            }
            _ => panic!("expected text"),
        }
        match items[1].annotation {
            Annotation::Symbols { on_move, on_position, prefix } => {
                assert_eq!(on_move, 1);
                assert_eq!(on_position, 2);
                assert_eq!(prefix, 0);
            }
            _ => panic!("expected symbols"),
        }

        assert_eq!(ann.texts(), vec!["A strong novelty!".to_string()]);
    }
}
