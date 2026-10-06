use crate::KazeNhanhError;
use pulldown_cmark::{CowStr, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use std::ops::Range;

/// Maps byte offsets to 1-based line numbers for Markdown documents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineOffsetMap {
    line_starts: Vec<usize>,
    text_len: usize,
}

impl LineOffsetMap {
    /// Builds a new offset map from the provided Markdown source.
    #[must_use]
    pub fn new(text: &str) -> Self {
        let mut line_starts = Vec::with_capacity(text.lines().count() + 1);
        line_starts.push(0);

        for (idx, ch) in text.char_indices() {
            if ch == '\n' {
                line_starts.push(idx + 1);
            }
        }

        Self {
            line_starts,
            text_len: text.len(),
        }
    }

    /// Returns the total number of lines represented in the source.
    #[must_use]
    pub fn total_lines(&self) -> u32 {
        if self.text_len == 0 {
            return 0;
        }

        self.line_starts
            .iter()
            .filter(|&&offset| offset < self.text_len)
            .count() as u32
    }

    /// Resolves the line number containing the supplied byte offset.
    pub fn line_for_offset(&self, offset: usize) -> Result<u32, KazeNhanhError> {
        if offset > self.text_len {
            return Err(KazeNhanhError::MarkdownParseError(format!(
                "byte offset {} exceeds document length {}",
                offset, self.text_len
            )));
        }

        let idx = match self.line_starts.binary_search(&offset) {
            Ok(i) => i,
            Err(i) => i.saturating_sub(1),
        };

        Ok((idx + 1) as u32)
    }

    /// Resolves the inclusive line span covered by the provided byte range.
    pub fn line_span(&self, range: Range<usize>) -> Result<(u32, u32), KazeNhanhError> {
        if range.start > range.end {
            return Err(KazeNhanhError::MarkdownParseError(format!(
                "invalid offset range: {}..{}",
                range.start, range.end
            )));
        }

        let start_line = self.line_for_offset(range.start)?;

        if range.start == range.end {
            return Ok((start_line, start_line));
        }

        let mut end_offset = range.end.saturating_sub(1);
        if end_offset > self.text_len {
            end_offset = self.text_len;
        }

        let end_line = self.line_for_offset(end_offset)?;

        Ok((start_line, end_line.max(start_line)))
    }
}

/// Represents a heading section extracted from a Markdown document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkdownSection {
    level: u8,
    title: String,
    start_line: u32,
    end_line: u32,
}

impl MarkdownSection {
    fn new(level: u8, title: String, start_line: u32) -> Self {
        Self {
            level,
            title,
            start_line,
            end_line: start_line,
        }
    }

    /// Returns the heading level (1-6) for the section.
    #[must_use]
    pub fn level(&self) -> u8 {
        self.level
    }

    /// Returns the normalized title text for the section heading.
    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Returns the inclusive starting line number for the section.
    #[must_use]
    pub fn start_line(&self) -> u32 {
        self.start_line
    }

    /// Returns the inclusive ending line number for the section.
    #[must_use]
    pub fn end_line(&self) -> u32 {
        self.end_line
    }
}

/// Traverses Markdown headings and produces structured section metadata.
#[derive(Debug)]
pub struct MarkdownService {
    options: Options,
}

impl MarkdownService {
    /// Creates a new service configured to emit source position metadata.
    #[must_use]
    pub fn new() -> Self {
        let options = Options::empty();

        Self { options }
    }

    /// Maps Markdown headings to section metadata, including line ranges.
    pub fn map_document_structure(
        &self,
        markdown: &str,
    ) -> Result<Vec<MarkdownSection>, KazeNhanhError> {
        if markdown.trim().is_empty() {
            return Ok(Vec::new());
        }

        let line_map = LineOffsetMap::new(markdown);
        let parser = Parser::new_ext(markdown, self.options);

        let mut sections: Vec<MarkdownSection> = Vec::new();
        let mut pending: Option<PendingHeading> = None;

        for (event, range) in parser.into_offset_iter() {
            match event {
                Event::Start(Tag::Heading { level, .. }) => {
                    finalize_pending(&mut pending, &line_map, &mut sections)?;
                    pending = Some(PendingHeading::new(level, range.start));
                }
                Event::End(TagEnd::Heading(level)) => {
                    if let Some(heading) = pending.take() {
                        if heading.level != level {
                            return Err(KazeNhanhError::MarkdownParseError(
                                "mismatched heading tags detected".to_string(),
                            ));
                        }

                        finalize_heading(heading, &line_map, &mut sections)?;
                    }
                }
                Event::Text(text)
                | Event::Code(text)
                | Event::Html(text)
                | Event::InlineHtml(text) => append_heading_text(&mut pending, &text),
                Event::SoftBreak | Event::HardBreak => {
                    if let Some(entry) = pending.as_mut() {
                        entry.raw_text.push(' ');
                    }
                }
                _ => {}
            }
        }

        if let Some(heading) = pending.take() {
            finalize_heading(heading, &line_map, &mut sections)?;
        }

        assign_section_ranges(&line_map, &mut sections);

        Ok(sections)
    }
}

impl Default for MarkdownService {
    fn default() -> Self {
        Self::new()
    }
}

struct PendingHeading {
    level: HeadingLevel,
    start_offset: usize,
    raw_text: String,
}

impl PendingHeading {
    fn new(level: HeadingLevel, start_offset: usize) -> Self {
        Self {
            level,
            start_offset,
            raw_text: String::new(),
        }
    }
}

fn finalize_pending(
    pending: &mut Option<PendingHeading>,
    line_map: &LineOffsetMap,
    sections: &mut Vec<MarkdownSection>,
) -> Result<(), KazeNhanhError> {
    if let Some(heading) = pending.take() {
        finalize_heading(heading, line_map, sections)?;
    }
    Ok(())
}

fn finalize_heading(
    heading: PendingHeading,
    line_map: &LineOffsetMap,
    sections: &mut Vec<MarkdownSection>,
) -> Result<(), KazeNhanhError> {
    let start_line = line_map.line_for_offset(heading.start_offset)?;
    let title = normalize_heading_text(&heading.raw_text);
    let level = heading_level_as_u8(heading.level);

    let section = MarkdownSection::new(level, title, start_line);
    sections.push(section);

    Ok(())
}

fn assign_section_ranges(line_map: &LineOffsetMap, sections: &mut [MarkdownSection]) {
    if sections.is_empty() {
        return;
    }

    let total_lines = line_map.total_lines();

    for idx in 0..sections.len() {
        let end_line = if idx + 1 < sections.len() {
            sections[idx + 1].start_line.saturating_sub(1)
        } else {
            total_lines.max(sections[idx].start_line)
        };

        if end_line >= sections[idx].start_line {
            sections[idx].end_line = end_line;
        }
    }
}

fn append_heading_text(pending: &mut Option<PendingHeading>, fragment: &CowStr<'_>) {
    if let Some(entry) = pending.as_mut() {
        if !entry.raw_text.is_empty() {
            entry.raw_text.push(' ');
        }
        entry.raw_text.push_str(fragment.as_ref());
    }
}

fn normalize_heading_text(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut prev_space = false;

    for ch in input.chars() {
        if ch.is_whitespace() {
            if !prev_space {
                result.push(' ');
                prev_space = true;
            }
        } else {
            result.push(ch);
            prev_space = false;
        }
    }

    result.trim().to_string()
}

fn heading_level_as_u8(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        assign_section_ranges, heading_level_as_u8, normalize_heading_text, LineOffsetMap,
        MarkdownSection, MarkdownService,
    };
    use pulldown_cmark::HeadingLevel;

    #[test]
    fn line_offset_map_tracks_line_counts() {
        let markdown = "# Title\nLine two\n\nLine four";
        let map = LineOffsetMap::new(markdown);

        assert_eq!(map.total_lines(), 4);
        assert_eq!(map.line_for_offset(0).unwrap(), 1);
        assert_eq!(map.line_for_offset(2).unwrap(), 1);
        assert_eq!(map.line_for_offset(8).unwrap(), 2);
        assert_eq!(map.line_for_offset(markdown.len()).unwrap(), 4);
    }

    #[test]
    fn line_offset_map_rejects_out_of_bounds_offsets() {
        let map = LineOffsetMap::new("# Heading\n");

        let error = map.line_for_offset(1024).unwrap_err();
        let message = error.to_string();

        assert!(message.contains("byte offset"));
    }

    #[test]
    fn heading_level_converts_to_u8() {
        assert_eq!(heading_level_as_u8(HeadingLevel::H1), 1);
        assert_eq!(heading_level_as_u8(HeadingLevel::H6), 6);
    }

    #[test]
    fn normalize_heading_collapses_whitespace() {
        let normalized = normalize_heading_text("  Foo   \n  Bar\tBaz ");
        assert_eq!(normalized, "Foo Bar Baz");
    }

    #[test]
    fn assigns_section_ranges_with_next_heading_boundaries() {
        let mut sections = vec![
            MarkdownSection::new(1, "Intro".to_string(), 1),
            MarkdownSection::new(2, "Details".to_string(), 5),
        ];

        let map = LineOffsetMap::new("# Intro\ntext\n\n## Details\ncontent\n");
        assign_section_ranges(&map, &mut sections);

        assert_eq!(sections[0].end_line(), 4);
        assert_eq!(sections[1].end_line(), 5);
    }

    #[test]
    fn markdown_service_maps_headings_to_sections() {
        let markdown = "# Title\nLead paragraph.\n\n## Sub\nContent\n\n### Deep\nMore";
        let service = MarkdownService::new();
        let sections = service
            .map_document_structure(markdown)
            .expect("mapping should succeed");

        assert_eq!(sections.len(), 3);
        assert_eq!(sections[0].level(), 1);
        assert_eq!(sections[0].title(), "Title");
        assert_eq!(sections[0].start_line(), 1);
        assert_eq!(sections[0].end_line(), 3);

        assert_eq!(sections[1].level(), 2);
        assert_eq!(sections[1].title(), "Sub");
        assert_eq!(sections[1].start_line(), 4);
        assert_eq!(sections[1].end_line(), 6);

        assert_eq!(sections[2].level(), 3);
        assert_eq!(sections[2].title(), "Deep");
        assert_eq!(sections[2].start_line(), 7);
        assert_eq!(sections[2].end_line(), 8);
    }

    #[test]
    fn markdown_service_handles_empty_input() {
        let service = MarkdownService::new();
        let sections = service.map_document_structure("   \n \n").unwrap();

        assert!(sections.is_empty());
    }
}
