use std::collections::BTreeMap;

use crate::foundation::git_service::LineAddition;
use crate::foundation::markdown::MarkdownSection;

use super::git_native_rag::DiffIngestionEntry;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ContextualAddition {
    pub line_number: u32,
    pub content: String,
}

impl From<&LineAddition> for ContextualAddition {
    fn from(addition: &LineAddition) -> Self {
        Self {
            line_number: addition.line_number,
            content: addition.content.clone(),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ContextualChange {
    pub file_path: String,
    pub section_title: Option<String>,
    pub section_level: Option<u8>,
    pub added_lines: Vec<ContextualAddition>,
}

pub(crate) fn correlate(entries: &[DiffIngestionEntry]) -> Vec<ContextualChange> {
    let mut correlated: Vec<ContextualChange> = Vec::new();

    for entry in entries {
        if entry.added_lines.is_empty() {
            continue;
        }

        let groups = group_by_section(entry);

        for (_key, group) in groups.into_iter() {
            correlated.push(group);
        }
    }

    correlated
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct SectionKey {
    start_line: u32,
    section_title: Option<String>,
    section_level: Option<u8>,
}

fn group_by_section(entry: &DiffIngestionEntry) -> BTreeMap<SectionKey, ContextualChange> {
    let mut grouped: BTreeMap<SectionKey, ContextualChange> = BTreeMap::new();

    for addition in &entry.added_lines {
        let section = find_section(&entry.sections, addition.line_number);
        let key = SectionKey::from_section(section);

        let context = grouped
            .entry(key.clone())
            .or_insert_with(|| ContextualChange {
                file_path: entry.file_path.clone(),
                section_title: key.section_title.clone(),
                section_level: key.section_level,
                added_lines: Vec::new(),
            });

        context.added_lines.push(ContextualAddition::from(addition));
    }

    grouped
}

fn find_section<'a>(sections: &'a [MarkdownSection], line: u32) -> Option<&'a MarkdownSection> {
    sections
        .iter()
        .filter(|section| {
            section.start_line() <= line && section.end_line().max(section.start_line()) >= line
        })
        .max_by_key(|section| section.start_line())
}

impl SectionKey {
    fn from_section(section: Option<&MarkdownSection>) -> Self {
        if let Some(section) = section {
            Self {
                start_line: section.start_line(),
                section_title: Some(section.title().to_string()),
                section_level: Some(section.level()),
            }
        } else {
            Self {
                start_line: 0,
                section_title: None,
                section_level: None,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::foundation::markdown::MarkdownService;

    fn make_entry(document: &str, added: &[(u32, &str)]) -> DiffIngestionEntry {
        let service = MarkdownService::new();
        let sections = service
            .map_document_structure(document)
            .expect("failed to parse document");

        DiffIngestionEntry {
            file_path: "docs/changes.md".to_string(),
            added_lines: added
                .iter()
                .map(|(line, text)| LineAddition {
                    line_number: *line,
                    content: text.to_string(),
                })
                .collect(),
            sections,
            document: document.to_string(),
        }
    }

    #[test]
    fn correlates_lines_with_matching_sections() {
        let document = "# Title\n\n## Summary\nNew insights\n\n## Details\nMore text\n";
        let entry = make_entry(
            document,
            &[(3, "- add summary bullet"), (6, "- add detail")],
        );

        let changes = correlate(&[entry]);

        assert_eq!(changes.len(), 2);

        let summary = changes
            .iter()
            .find(|change| change.section_title.as_deref() == Some("Summary"))
            .expect("missing summary change");
        assert_eq!(summary.added_lines.len(), 1);
        assert_eq!(summary.added_lines[0].line_number, 3);

        let details = changes
            .iter()
            .find(|change| change.section_title.as_deref() == Some("Details"))
            .expect("missing details change");
        assert_eq!(details.added_lines[0].line_number, 6);
    }

    #[test]
    fn assigns_lines_without_section_to_default_group() {
        let document = "Intro text\nSecond line\n";
        let entry = make_entry(document, &[(1, "- new intro line"), (2, "- another line")]);

        let changes = correlate(&[entry]);

        assert_eq!(changes.len(), 1);
        let change = &changes[0];
        assert!(change.section_title.is_none());
        assert_eq!(change.added_lines.len(), 2);
        assert_eq!(change.added_lines[0].content, "- new intro line");
    }

    #[test]
    fn collapses_multiple_lines_in_same_section() {
        let document = "# Title\n\n## Summary\nLine\n";
        let entry = make_entry(document, &[(3, "- first"), (4, "- second")]);

        let changes = correlate(&[entry]);

        assert_eq!(changes.len(), 1);
        let summary = &changes[0];
        assert_eq!(summary.section_title.as_deref(), Some("Summary"));
        assert_eq!(summary.added_lines.len(), 2);
    }
}
