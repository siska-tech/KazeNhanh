use std::collections::BTreeMap;

use super::correlation_engine::ContextualChange;

const FALLBACK_SECTION_TITLE: &str = "(no heading)";

pub(crate) fn build_prompt(
    repo_path: &str,
    days_since: u32,
    changes: &[ContextualChange],
    custom_prompt: Option<&str>,
) -> String {
    let mut prompt = String::new();

    prompt.push_str(
        "You are an AI assistant that reviews Git markdown changes and produces actionable insights.\n",
    );
    prompt.push_str(&format!(
        "Repository: {}\nTime Window: Last {} day(s)\n\n",
        repo_path.trim(),
        days_since
    ));

    if let Some(instructions) = custom_prompt.and_then(|text| {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    }) {
        prompt.push_str("Additional Analyst Guidance:\n");
        prompt.push_str(instructions);
        prompt.push_str("\n\n");
    }

    prompt.push_str("Summarize the following contextualized changes. Focus on intent, potential impact, and highlight any documentation or testing gaps.\n\n");

    let mut grouped: BTreeMap<&str, Vec<&ContextualChange>> = BTreeMap::new();
    for change in changes {
        grouped
            .entry(change.file_path.as_str())
            .or_default()
            .push(change);
    }

    for (file_path, clustered) in grouped {
        prompt.push_str(&format!("File: {}\n", file_path));

        for change in clustered {
            let heading = change
                .section_title
                .as_deref()
                .unwrap_or(FALLBACK_SECTION_TITLE);
            prompt.push_str(&format!("  Section: {}", heading));
            if let Some(level) = change.section_level {
                prompt.push_str(&format!(" (level {})", level));
            }
            prompt.push('\n');

            for line in &change.added_lines {
                prompt.push_str(&format!(
                    "    +{} {}\n",
                    line.line_number,
                    line.content.trim()
                ));
            }

            prompt.push('\n');
        }
    }

    prompt.trim_end().to_string()
}

pub(crate) fn fallback_message(repo_path: &str, days_since: u32) -> String {
    format!(
        "No relevant Markdown additions detected for `{}` within the last {} day(s).",
        repo_path.trim(),
        days_since
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::correlation_engine::ContextualAddition;

    fn sample_change(section: Option<&str>) -> ContextualChange {
        ContextualChange {
            file_path: "docs/guide.md".to_string(),
            section_title: section.map(|s| s.to_string()),
            section_level: section.map(|_| 2),
            added_lines: vec![ContextualAddition {
                line_number: 12,
                content: "- new bullet".to_string(),
            }],
        }
    }

    #[test]
    fn build_prompt_includes_custom_guidance_and_changes() {
        let changes = vec![sample_change(Some("Summary"))];

        let prompt = build_prompt("repo", 3, &changes, Some("Emphasize user-facing impact."));

        assert!(prompt.contains("Repository: repo"));
        assert!(prompt.contains("Additional Analyst Guidance"));
        assert!(prompt.contains("Summary"));
        assert!(prompt.contains("- new bullet"));
    }

    #[test]
    fn build_prompt_handles_missing_headings() {
        let changes = vec![sample_change(None)];

        let prompt = build_prompt("repo", 5, &changes, None);

        assert!(prompt.contains(FALLBACK_SECTION_TITLE));
    }

    #[test]
    fn fallback_message_formats_repository_and_window() {
        let message = fallback_message("repo", 7);

        assert!(message.contains("repo"));
        assert!(message.contains("7"));
    }
}
