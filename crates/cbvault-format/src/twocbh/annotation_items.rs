// MIT-licensed annotation layouts adapted from oschess-cb-bridge v1.0.3,
// commit bffebfc498d52b4e4631aed32b2f8e8194d125ff.
// Copyright (c) 2026 the oschess-cb-bridge contributors.
// Modifications copyright (c) 2026 the cbvault contributors.
// SPDX-License-Identifier: MIT
//! Borrowed, allocation-free v2 annotation items. Unknown item layouts are
//! explicit errors rather than silently discarded or attached to wrong plies.
//! The game comment position is -1; other positions count moves in stored order.

use std::path::Path;
use crate::error::{Error, Result};
use crate::game::annotations::{Annotation, GAME_POSITION};
use super::record::Record;

const END: i32 = 0x7fff_ffff;
const ANNOTATION_TAG: u16 = 0x2000;

/// A complete validated v2 annotation record, borrowing its framed payload.
#[derive(Debug)]
pub struct GameAnnotations<'a> {
    path: &'a Path,
    content: &'a [u8],
    count: u32,
}
/// One v2 annotation, with its position and byte offset in the content.
#[derive(Clone, Copy, Debug)]
pub struct Item<'a> {
    /// Stored-order ply index, or -1 for the whole-game comment.
    pub position: i32,
    /// Byte offset of the type code in the annotation content.
    pub offset: u32,
    /// Fully parsed borrowed item.
    pub annotation: Annotation<'a>,
}

/// Bounds-checked cursor. Its methods allocate only to report corrupt input.
#[derive(Clone)]
struct Reader<'a> {
    data: &'a [u8],
    at: usize,
    path: &'a Path,
}

impl<'a> Reader<'a> {
    fn new(path: &'a Path, data: &'a [u8]) -> Self { Self { data, at: 0, path } }
    fn bad(&self, reason: impl Into<String>) -> Error { Error::corrupt(self.path, self.at as u64, reason) }
    fn left(&self) -> usize { self.data.len() - self.at }
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self.at.checked_add(n).ok_or_else(|| self.bad("2CBA: length overflow"))?;
        let slice = self.data.get(self.at..end).ok_or_else(|| self.bad("2CBA: truncated annotation item"))?;
        self.at = end;
        Ok(slice)
    }
    fn skip(&mut self, n: usize) -> Result<()> { self.take(n).map(|_| ()) }
    fn u8(&mut self) -> Result<u8> { Ok(self.take(1)?[0]) }
    fn u16(&mut self) -> Result<u16> { Ok(u16::from_le_bytes(self.take(2)?.try_into().expect("two bytes"))) }
    fn i32(&mut self) -> Result<i32> { Ok(i32::from_le_bytes(self.take(4)?.try_into().expect("four bytes"))) }
    fn len(&mut self) -> Result<usize> {
        let n = self.i32()?;
        usize::try_from(n).ok().filter(|&n| n <= self.left())
            .ok_or_else(|| self.bad("2CBA: invalid length-prefixed item"))
    }
    fn expect_one(&mut self) -> Result<()> {
        if self.u8()? == 1 { Ok(()) } else { Err(self.bad("2CBA: expected marker 01")) }
    }
    fn text_record(&mut self) -> Result<&'a [u8]> {
        let n = self.len()?;
        self.take(n)
    }
    fn training_list(&mut self) -> Result<()> {
        let count = self.u16()?;
        for _ in 0..count {
            self.skip(2)?;
            let n = self.len()?;
            self.skip(n)?;
        }
        Ok(())
    }
    fn training(&mut self) -> Result<()> {
        let head = self.take(6)?;
        let variant = head[2];
        if !matches!(variant, 1 | 2) {
            return Err(self.bad(format!("2CBA: unsupported training variant {variant}")));
        }
        self.skip(6)?;
        for _ in 0..4 { self.training_list()?; }
        let count = self.u8()?;
        for _ in 0..count {
            if variant == 1 {
                self.skip(4)?;
                self.training_list()?;
            } else {
                self.skip(1)?;
                self.training_list()?;
                self.training_list()?;
            }
        }
        Ok(())
    }
    fn quotation(&mut self) -> Result<()> {
        self.expect_one()?;
        self.skip(9)?;
        for _ in 0..6 {
            let len = self.u8()? as usize;
            self.skip(len)?;
        }
        self.skip(35 + 44)?;
        for _ in 0..2 {
            self.skip(5)?;
            let len = self.len()?;
            self.skip(len)?;
        }
        self.skip(26)?;
        match self.u8()? {
            1 => {}
            0 => self.skip(64 + 11)?,
            _ => return Err(self.bad("2CBA: unknown quoted-game start marker")),
        }
        self.skip(2)?;
        let count = self.len()?;
        let bytes = count.checked_mul(5).ok_or_else(|| self.bad("2CBA: quoted game move count overflow"))?;
        self.skip(bytes)?;
        self.skip(4)
    }
    fn annotation(&mut self, kind: u16) -> Result<Annotation<'a>> {
        let begin = self.at;
        let result = match kind {
            0x02 | 0x82 => {
                self.skip(2)?;
                let language = self.u16()?;
                let text = self.text_record()?;
                Annotation::Text { before: kind == 0x82, language, text }
            }
            0x03 => {
                let b = self.take(3)?;
                Annotation::Symbols { on_move: b[0], on_position: b[1], prefix: b[2] }
            }
            0x04 => {
                let items = self.text_record()?;
                if items.len() % 2 != 0 || items.as_chunks::<2>().0.iter().any(|p| !(1..=64).contains(&p[1])) {
                    return Err(self.bad("2CBA: invalid coloured square array"));
                }
                Annotation::Squares(items)
            }
            0x05 => {
                let items = self.text_record()?;
                if items.len() % 3 != 0 ||
                    items.as_chunks::<3>().0.iter().any(|p| !(1..=64).contains(&p[1]) || !(1..=64).contains(&p[2]))
                {
                    return Err(self.bad("2CBA: invalid arrow array"));
                }
                Annotation::Arrows(items)
            }
            0x15 => { let n = self.len()?; self.skip(n)?; Annotation::Other { code: kind, data: &self.data[begin..self.at] } }
            0x07 | 0x08 | 0x16 | 0x17 | 0x22 | 0x23 | 0x25 => {
                self.skip(4)?;
                Annotation::Other { code: kind, data: &self.data[begin..self.at] }
            }
            0x27 => { self.skip(2)?; Annotation::Other { code: kind, data: &self.data[begin..self.at] } }
            0x14 | 0x18 => { self.skip(1)?; Annotation::Other { code: kind, data: &self.data[begin..self.at] } }
            0x21 => { self.skip(6)?; Annotation::Other { code: kind, data: &self.data[begin..self.at] } }
            0x24 => { self.skip(38)?; Annotation::Other { code: kind, data: &self.data[begin..self.at] } }
            0x26 => {
                self.expect_one()?;
                let n = self.len()?;
                self.skip(n)?;
                Annotation::Other { code: kind, data: &self.data[begin..self.at] }
            }
            0x09 => {
                self.training()?;
                Annotation::Other { code: kind, data: &self.data[begin..self.at] }
            }
            0x13 => {
                self.quotation()?;
                Annotation::Other { code: kind, data: &self.data[begin..self.at] }
            }
            0x20 => {
                self.skip(4)?;
                let n = self.len()?;
                self.skip(n)?;
                Annotation::Other { code: kind, data: &self.data[begin..self.at] }
            }
            0x1c => {
                self.expect_one()?;
                for _ in 0..2 {
                    let n = self.len()?;
                    self.skip(n)?;
                }
                Annotation::Other { code: kind, data: &self.data[begin..self.at] }
            }
            _ => return Err(self.bad(format!("2CBA: unsupported annotation type 0x{kind:04x}"))),
        };
        Ok(result)
    }
}

impl<'a> GameAnnotations<'a> {
    /// Parse a whole framed annotation record and validate all position blocks
    /// and known types before exposing them as borrowed items.
    pub fn parse(path: &'a Path, framed: &'a [u8]) -> Result<Self> {
        let frame = Record::parse(path, 0, framed)?;
        if frame.tag() != ANNOTATION_TAG {
            return Err(Error::corrupt(path, 0x18, "2CBA: frame tag is not 0x2000"));
        }
        Self::from_content(path, frame.content())
    }

    /// Parse a verified annotation payload without any copying.
    pub fn from_content(path: &'a Path, content: &'a [u8]) -> Result<Self> {
        let mut reader = Reader::new(path, content);
        let mut total = 0u32;
        loop {
            let position = reader.i32()?;
            if position == END {
                if reader.left() != 0 {
                    return Err(reader.bad("2CBA: bytes after final position sentinel"));
                }
                break;
            }
            if position < GAME_POSITION {
                return Err(reader.bad("2CBA: invalid negative annotation position"));
            }
            let count = reader.i32()?;
            if count < 0 || count as usize > reader.left() / 2 {
                return Err(reader.bad("2CBA: annotation count exceeds available bytes"));
            }
            total = total.checked_add(count as u32)
                .ok_or_else(|| reader.bad("2CBA: annotation count overflow"))?;
            for _ in 0..count {
                let typ = reader.u16()?;
                reader.annotation(typ)?;
            }
        }
        Ok(Self { path, content, count: total })
    }

    /// Number of annotation items.
    pub fn count(&self) -> u32 { self.count }
    /// Whether this game has no annotations.
    pub fn is_empty(&self) -> bool { self.count == 0 }
    /// Iterate borrowed annotation items in on-disk order.
    pub fn iter(&self) -> Items<'a> { Items { reader: Reader::new(self.path, self.content), position: -1, left: 0, complete: false } }
    /// Parse the item at the byte offset emitted by `iter()`.
    pub fn annotation_at(&self, offset: u32) -> Result<Annotation<'a>> {
        let mut reader = Reader::new(self.path, self.content);
        reader.at = offset as usize;
        let typ = reader.u16()?;
        reader.annotation(typ)
    }
    /// Ensure no annotations are silently placed after the game's final ply.
    pub fn check_positions(&self, plies: u32) -> Result<usize> {
        let mut out_of_range = 0;
        for item in self.iter() {
            let item = item?;
            if item.position >= 0 && plies == 0 {
                return Err(Error::corrupt(self.path, item.offset as u64, "2CBA: move-specific comment in move-less game"));
            }
            if item.position >= plies as i32 { out_of_range += 1; }
        }
        Ok(out_of_range)
    }
}

/// Iterator of position groups and their typed annotation items. No Vec,
/// String or per-item allocation on valid data.
pub struct Items<'a> {
    reader: Reader<'a>,
    position: i32,
    left: u32,
    complete: bool,
}

impl<'a> Iterator for Items<'a> {
    type Item = Result<Item<'a>>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.complete { return None; }
        if self.left == 0 {
            loop {
                let position = match self.reader.i32() {
                    Ok(p) => p,
                    Err(e) => { self.complete = true; return Some(Err(e)); }
                };
                if position == END { self.complete = true; return None; }
                let count = match self.reader.i32() {
                    Ok(c) => c,
                    Err(e) => { self.complete = true; return Some(Err(e)); }
                };
                if count < 0 {
                    self.complete = true;
                    return Some(Err(self.reader.bad("2CBA: invalid annotation count")));
                }
                self.position = position;
                self.left = count as u32;
                if self.left > 0 { break; }
            }
        }
        let offset = self.reader.at as u32;
        let result = self.reader.u16().and_then(|code| self.reader.annotation(code)).map(|annotation| {
            Item { position: self.position, offset, annotation }
        });
        self.left -= 1;
        if result.is_err() { self.complete = true; }
        Some(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_and_simple_synthetic_annotations() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&(-1i32).to_le_bytes());
        bytes.extend_from_slice(&1i32.to_le_bytes());
        bytes.extend_from_slice(&3u16.to_le_bytes());
        bytes.extend_from_slice(&[1, 0, 0]);
        bytes.extend_from_slice(&END.to_le_bytes());
        let a = GameAnnotations::from_content(Path::new("test.2cba"), &bytes).unwrap();
        assert_eq!(a.count(), 1);
        let one = a.iter().next().unwrap().unwrap();
        assert_eq!(one.position, -1);
        assert!(matches!(one.annotation, Annotation::Symbols { on_move: 1, .. }));
        assert_eq!(a.annotation_at(one.offset).unwrap(), one.annotation);
    }
    #[test]
    fn unknown_layout_cannot_be_silently_skipped() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&0i32.to_le_bytes());
        bytes.extend_from_slice(&1i32.to_le_bytes());
        bytes.extend_from_slice(&0xfffeu16.to_le_bytes());
        bytes.extend_from_slice(&END.to_le_bytes());
        let e = GameAnnotations::from_content(Path::new("test.2cba"), &bytes).unwrap_err();
        assert!(e.to_string().contains("unsupported annotation type"));
    }
}
