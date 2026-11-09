// Copyright (c) 2026 the oschess-cb-bridge contributors
// Modified for cbvault (MIT License). See docs/provenance.md.

//! `.2cba` records: the annotations of one game.
//!
//! A record is a run of position blocks, each starting with `position: i32`
//! (−1 for the game as a whole, or the ply index) and `count: i32`, followed by
//! `count` annotations, terminated by the marker `0x7fffffff`.
//!
//! Each annotation starts with a 2-byte type code (`u16 LE`). Unknown types
//! produce a typed iterator error, rather than silently truncating the
//! record. The iterator retains `stopped_at` for diagnostic callers.

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

    /// Wraps an already validated `.2cba` record frame.
    #[inline]
    pub fn from_record(record: Record<'a>) -> Self {
        Annotations { record }
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

    /// Whether the record carries no annotations.
    pub fn is_empty(&self) -> bool {
        self.content().is_empty()
            || (self.content().len() == 4 && i32::from_le_bytes(self.content()[..4].try_into().unwrap()) == END_MARKER)
    }

    /// Returns the annotation at byte `offset` within the record's content.
    pub fn annotation_at(&self, offset: u32) -> Result<Annotation<'a>> {
        let content = self.content();
        let off = offset as usize;
        if off + 2 > content.len() {
            return Err(Error::corrupt("<.2cba>", off as u64, "truncated annotation type"));
        }
        let type_code = u16::from_le_bytes(content[off..off + 2].try_into().unwrap());
        let mut cur = off + 2;
        match parse_annotation(content, &mut cur, type_code)? {
            Some(a) => Ok(a),
            None => Err(Error::corrupt("<.2cba>", off as u64, format!("unknown annotation type {type_code:#04x}"))),
        }
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
        if self.stopped {
            return None;
        }
        if self.offset >= self.content.len() {
            self.stopped = true;
            // Empty annotation records are valid; a nonempty one must end
            // in the documented END_MARKER, not an implicit EOF.
            return (!self.content.is_empty())
                .then(|| Err(Error::corrupt("<.2cba>", self.offset as u64, "missing 2CBA end marker")));
        }

        while self.remaining_in_block == 0 {
            if self.offset + 4 > self.content.len() {
                self.stopped = true;
                return Some(Err(Error::corrupt("<.2cba>", self.offset as u64, "truncated 2CBA position/end marker")));
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
                // The item boundary after an unknown kind is not known. Stopping
                // silently would make every caller treat a truncated annotated
                // game as fully decoded, so always surface a typed failure.
                self.stopped = true;
                self.stopped_at = Some((self.curr_pos, type_code));
                Some(Err(Error::corrupt(
                    "<.2cba>",
                    u64::from(item_offset),
                    format!("unknown 2CBA annotation type 0x{type_code:04x} at position {}", self.curr_pos),
                )))
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
        let Some(end) = self.offset.checked_add(n) else {
            return Err(self.bad("annotation offset overflow"));
        };
        if end > self.content.len() {
            return Err(self.bad("annotation runs past content"));
        }
        let s = &self.content[self.offset..end];
        self.offset = end;
        Ok(s)
    }

    fn i32_val(&mut self) -> Result<i32> {
        let b = self.take(4)?;
        Ok(i32::from_le_bytes(b.try_into().unwrap()))
    }

    fn len_val(&mut self) -> Result<usize> {
        let v = self.i32_val()?;
        usize::try_from(v).map_err(|_| self.bad("negative length in annotation"))
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
            let n = r.len_val()?;
            let text_bytes = r.take(n)?;
            Ok(Some(Annotation::Text { before: t == 0x82, language, text: text_bytes }))
        }
        0x03 => {
            let b = r.take(3)?;
            Ok(Some(Annotation::Symbols { on_move: b[0], on_position: b[1], prefix: b[2] }))
        }
        0x04 => {
            let n = r.len_val()?;
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
            let n = r.len_val()?;
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
        0x26 => {
            let start = r.offset;
            let _marker = r.take(1)?;
            let len = r.len_val()?;
            let _ = r.take(len)?;
            let total = r.offset - start;
            Ok(Some(Annotation::Other { code: t, data: &content[start..start + total] }))
        }
        0x13 => {
            let start = r.offset;
            if r.content.get(r.offset) == Some(&1) {
                r.take(10)?;
                for _ in 0..6 {
                    let sl = r.take(1)?[0] as usize;
                    r.take(sl)?;
                }
                r.take(53)?;
                r.take(8)?;
                r.take(18)?;
                r.take(5)?;
                let l1 = r.len_val()?;
                r.take(l1)?;
                r.take(5)?;
                let l2 = r.len_val()?;
                r.take(l2)?;
                r.take(18)?;
                let sl = r.len_val()?;
                r.take(sl)?;
                r.take(3)?;
                let move_count = r.len_val()?;
                r.take(move_count * 5)?;
                if r.offset + 4 <= r.content.len() && r.content[r.offset..r.offset + 4] == [0, 0, 0, 0] {
                    r.take(4)?;
                }
            } else {
                for _ in 0..6 {
                    let len = r.len_val()?;
                    let _ = r.take(len)?;
                }
                let _ = r.take(35)?;
                for _ in 0..2 {
                    let len = r.len_val()?;
                    let _ = r.take(len)?;
                }
                let _ = r.take(29)?;
                let _ = r.take(5)?;
            }
            let total = r.offset - start;
            Ok(Some(Annotation::Other { code: t, data: &content[start..start + total] }))
        }
        0x15 => {
            let n = r.len_val()?;
            let data = r.take(n)?;
            Ok(Some(Annotation::Other { code: t, data }))
        }
        0x20 => {
            let prefix = r.take(4)?;
            let n = r.len_val()?;
            let _body = r.take(n)?;
            let start = prefix.as_ptr() as usize - content.as_ptr() as usize;
            let len = 4 + 4 + n;
            Ok(Some(Annotation::Other { code: t, data: &content[start..start + len] }))
        }
        0x1c => {
            let start = r.offset;
            let _marker = r.take(1)?;
            let url_len = r.len_val()?;
            let _ = r.take(url_len)?;
            let desc_len = r.len_val()?;
            let _ = r.take(desc_len)?;
            let total = r.offset - start;
            Ok(Some(Annotation::Other { code: t, data: &content[start..start + total] }))
        }
        0x09 => {
            let start = r.offset;
            r.take(2)?; // marker [1, 1]
            r.i32_val()?; // field1
            r.i32_val()?; // time_limit
            r.u16_val()?; // points
            let q_cnt = r.u16_val()? as usize;
            for _ in 0..q_cnt {
                r.u16_val()?; // lang
                let q_len = r.len_val()?;
                r.take(q_len)?;
            }
            let extra_cnt = r.u16_val()? as usize;
            for _ in 0..extra_cnt {
                r.u16_val()?; // lang
                let n = r.len_val()?;
                r.take(n)?;
            }
            for _ in 0..2 {
                let hint_cnt = r.u16_val()? as usize;
                for _ in 0..hint_cnt {
                    r.u16_val()?; // lang
                    let n = r.len_val()?;
                    r.take(n)?;
                }
            }
            let sol_cnt = r.take(1)?[0] as usize;
            for _ in 0..sol_cnt {
                r.take(3)?; // move
                r.take(1)?; // points
                let fb_cnt = r.u16_val()? as usize;
                for _ in 0..fb_cnt {
                    r.u16_val()?; // tag
                    let fb_len = r.len_val()?;
                    r.take(fb_len)?;
                }
            }
            let total = r.offset - start;
            Ok(Some(Annotation::Other { code: t, data: &content[start..start + total] }))
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

    #[test]
    fn unknown_annotation_kind_must_not_look_like_success() {
        let mut body = Vec::new();
        body.extend(0i32.to_le_bytes());
        body.extend(1i32.to_le_bytes());
        body.extend(0x7777u16.to_le_bytes());
        body.extend(END_MARKER.to_le_bytes());
        let raw = record_with_tag(ANNOTATION_TAG, body.len() as u32, 0, &body);
        let ann = Annotations::parse(Path::new("unknown.2cba"), &raw).unwrap();
        let mut items = ann.iter_items();
        let e = items.next().expect("unknown kind must be reported").unwrap_err();
        assert!(e.to_string().contains("0x7777"), "{e}");
        assert_eq!(items.stopped_at, Some((0, 0x7777)));
        assert!(items.next().is_none());
        assert!(ann.check_positions(5).is_err());
    }

    #[test]
    fn missing_end_marker_is_a_failure_not_successful_eof() {
        let mut body = Vec::new();
        body.extend(0i32.to_le_bytes());
        body.extend(1i32.to_le_bytes());
        body.extend(0x03u16.to_le_bytes());
        body.extend([1u8, 0, 0]);
        let raw = record_with_tag(ANNOTATION_TAG, body.len() as u32, 0, &body);
        let ann = Annotations::parse(Path::new("truncated.2cba"), &raw).unwrap();
        let mut items = ann.iter_items();
        assert!(items.next().expect("one item").is_ok());
        assert!(items.next().expect("missing terminator").is_err());
        assert!(items.next().is_none());
    }

    #[test]
    fn parse_complex_annotations() {
        let mut body = Vec::new();
        // Position -1, 3 annotations
        body.extend((-1i32).to_le_bytes());
        body.extend(3i32.to_le_bytes());

        // 1. Time spent: 0x07 (flags/hundredths, sec=10, min=2, hr=1)
        body.extend(0x07u16.to_le_bytes());
        body.extend([0, 10, 2, 1]);

        // 2. Evaluation curve: 0x26 (0x01, length=8, 2 evaluations)
        body.extend(0x26u16.to_le_bytes());
        body.push(0x01);
        body.extend(8i32.to_le_bytes());
        body.extend([0x10, 0x00, 0x00, 0x00, 0x20, 0x00, 0x00, 0x00]);

        // 3. Quotation: 0x13
        // 6 length-prefixed strings, 35 bytes binary, 2 strings, 29 bytes, 5 bytes
        body.extend(0x13u16.to_le_bytes());
        for s in &["White", "Black", "Tournament", "Place", "Annotator", "Source"] {
            body.extend((s.len() as i32).to_le_bytes());
            body.extend_from_slice(s.as_bytes());
        }
        body.extend([0u8; 35]);
        for r in &["2800", "2750"] {
            body.extend((r.len() as i32).to_le_bytes());
            body.extend_from_slice(r.as_bytes());
        }
        body.extend([0u8; 29]);
        body.extend([0u8; 5]);

        body.extend(END_MARKER.to_le_bytes());

        let raw = record_with_tag(ANNOTATION_TAG, body.len() as u32, 0, &body);
        let ann = Annotations::parse(Path::new("test_complex.2cba"), &raw).expect("parse annotations");
        let items: Vec<_> = ann.iter_items().collect::<Result<_>>().unwrap();
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].annotation.kind(), crate::game::annotations::Kind::TimeSpent);
        assert_eq!(items[1].annotation.kind(), crate::game::annotations::Kind::Evaluations);
        assert_eq!(items[2].annotation.kind(), crate::game::annotations::Kind::Quotation);
    }
}
