use std::{collections::BTreeMap, fmt};

use crate::skill::Agent;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ParsedSkillMarkdown {
    pub description: Option<String>,
    pub frontmatter: BTreeMap<String, String>,
    pub agents: Vec<Agent>,
    pub referenced_resources: Vec<String>,
}

pub fn parse_skill_markdown(content: &str) -> Result<ParsedSkillMarkdown, ParseError> {
    if content.trim().is_empty() {
        return Err(ParseError::Empty);
    }

    let (frontmatter, body) = split_frontmatter(content);
    let description = frontmatter
        .get("description")
        .cloned()
        .or_else(|| first_body_paragraph(body));
    let agents = parse_agents(&frontmatter);
    let referenced_resources = parse_referenced_resources(content);

    Ok(ParsedSkillMarkdown {
        description,
        frontmatter,
        agents,
        referenced_resources,
    })
}

fn split_frontmatter(content: &str) -> (BTreeMap<String, String>, &str) {
    let Some(rest) = content.strip_prefix("---\n") else {
        return (BTreeMap::new(), content);
    };
    let Some(end) = rest.find("\n---") else {
        return (BTreeMap::new(), content);
    };

    let frontmatter = &rest[..end];
    let body = rest[end + "\n---".len()..].trim_start_matches(['\r', '\n']);

    (parse_frontmatter(frontmatter), body)
}

fn parse_frontmatter(frontmatter: &str) -> BTreeMap<String, String> {
    let mut values = BTreeMap::new();
    let mut lines = frontmatter.lines().peekable();

    while let Some(line) = lines.next() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let key = key.trim().to_string();
        let value = value.trim();

        if matches!(value, "|" | ">") {
            let mut block = Vec::new();
            while let Some(next) = lines.peek() {
                if !next.starts_with(' ') && !next.starts_with('\t') && !next.trim().is_empty() {
                    break;
                }
                block.push(lines.next().unwrap().trim().to_string());
            }
            values.insert(key, block.join(" ").trim().to_string());
        } else {
            values.insert(key, unquote(value).to_string());
        }
    }

    values
}

fn unquote(value: &str) -> &str {
    value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .or_else(|| {
            value
                .strip_prefix('\'')
                .and_then(|value| value.strip_suffix('\''))
        })
        .unwrap_or(value)
}

fn first_body_paragraph(body: &str) -> Option<String> {
    body.lines()
        .map(str::trim)
        .find(|line| {
            !line.is_empty()
                && !line.starts_with('#')
                && !line.starts_with("```")
                && !line.starts_with("- ")
        })
        .map(ToString::to_string)
}

fn parse_agents(frontmatter: &BTreeMap<String, String>) -> Vec<Agent> {
    let Some(value) = frontmatter.get("agents") else {
        return Vec::new();
    };

    value
        .trim_matches(['[', ']'])
        .split(',')
        .map(str::trim)
        .map(unquote)
        .filter(|name| !name.is_empty())
        .map(Agent::enabled)
        .collect()
}

fn parse_referenced_resources(content: &str) -> Vec<String> {
    let mut resources = Vec::new();
    collect_markdown_links(content, &mut resources);
    collect_bare_reference_paths(content, &mut resources);
    resources.sort();
    resources.dedup();
    resources
}

fn collect_markdown_links(content: &str, resources: &mut Vec<String>) {
    let mut rest = content;
    while let Some(start) = rest.find("](") {
        rest = &rest[start + 2..];
        let Some(end) = rest.find(')') else {
            break;
        };
        let target = rest[..end].trim();
        if is_local_resource(target) {
            resources.push(target.to_string());
        }
        rest = &rest[end + 1..];
    }
}

fn collect_bare_reference_paths(content: &str, resources: &mut Vec<String>) {
    for token in content.split(|character: char| character.is_whitespace() || character == '`') {
        let token = token.trim_matches(|character| {
            matches!(character, ',' | '.' | ')' | '(' | ']' | '[' | '"' | '\'')
        });
        if token.contains("](") || token.contains("://") {
            continue;
        }
        if is_local_resource(token) {
            resources.push(token.to_string());
        }
    }
}

fn is_local_resource(target: &str) -> bool {
    !target.starts_with("http://")
        && !target.starts_with("https://")
        && (target.starts_with("references/")
            || target.starts_with("assets/")
            || target.starts_with("scripts/")
            || target.ends_with(".md")
            || target.ends_with(".sh"))
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ParseError {
    Empty,
}

impl fmt::Display for ParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(formatter, "SKILL.md is empty"),
        }
    }
}

impl std::error::Error for ParseError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_frontmatter_description_agents_and_resources() {
        let parsed = parse_skill_markdown(
            r#"---
name: code-review
description: "Review pull requests"
agents: ["reviewer", "tester"]
---

# Code Review

Read [guide](references/review.md) and run `scripts/check.sh`.
"#,
        )
        .unwrap();

        assert_eq!(parsed.description.as_deref(), Some("Review pull requests"));
        assert_eq!(parsed.frontmatter.get("name").unwrap(), "code-review");
        assert_eq!(parsed.agents.len(), 2);
        assert_eq!(parsed.agents[0].name, "reviewer");
        assert_eq!(
            parsed.referenced_resources,
            vec!["references/review.md", "scripts/check.sh"]
        );
    }

    #[test]
    fn falls_back_to_first_body_paragraph() {
        let parsed = parse_skill_markdown(
            r#"# Browser Skill

Automate browser workflows.

- list item
"#,
        )
        .unwrap();

        assert_eq!(
            parsed.description.as_deref(),
            Some("Automate browser workflows.")
        );
    }

    #[test]
    fn parses_block_scalar_description() {
        let parsed = parse_skill_markdown(
            r#"---
description: |
  First line.
  Second line.
---

# Skill
"#,
        )
        .unwrap();

        assert_eq!(
            parsed.description.as_deref(),
            Some("First line. Second line.")
        );
    }

    #[test]
    fn ignores_remote_links() {
        let parsed = parse_skill_markdown(
            r#"# Skill

See [remote](https://example.com/guide.md) and [local](assets/icon.txt).
"#,
        )
        .unwrap();

        assert_eq!(parsed.referenced_resources, vec!["assets/icon.txt"]);
    }

    #[test]
    fn rejects_empty_skill_markdown() {
        assert_eq!(parse_skill_markdown(" \n ").unwrap_err(), ParseError::Empty);
    }
}
