//! The portable generation contract. Existing stored drafts are never migrated
//! or revalidated here; these checks apply to a new model response only.
use anyhow::{Result, bail};
use std::collections::{HashMap, HashSet};

pub const RULES: &str = include_str!("../prompts/design-document-format.md");
const SECTIONS: [&str; 4] = ["Navigation", "Jobflow", "State", "Screen Layout"];
const LANGUAGES: [&str; 4] = ["navigation", "jobflow", "state", "layout"];

fn invalid(detail: &str) -> anyhow::Error {
    anyhow::anyhow!("architecture document format: {detail}")
}

/// Check observable structure and the conservative DSL subset documented in
/// RULES. This is not a proof of project semantics or visual rendering quality.
pub fn validate(markdown: &str) -> Result<()> {
    if !markdown.starts_with("---\n") {
        return Err(invalid("SmartMD frontmatter must start at the first byte"));
    }
    let clean = without_comments(markdown)?;
    let lines: Vec<_> = clean.lines().collect();
    if lines.first() != Some(&"---") {
        return Err(invalid("SmartMD frontmatter must start on the first line"));
    }
    let end = lines
        .iter()
        .skip(1)
        .position(|l| *l == "---")
        .map(|p| p + 1)
        .ok_or_else(|| invalid("unclosed frontmatter"))?;
    let fields: HashMap<_, _> = lines[1..end]
        .iter()
        .filter_map(|line| {
            if line.starts_with(char::is_whitespace) {
                return None;
            }
            let (key, value) = line.split_once(':')?;
            Some((key.trim(), value.trim().trim_matches(['\'', '"'])))
        })
        .collect();
    if fields.get("title").is_none_or(|v| v.is_empty())
        || !matches!(fields.get("locale"), Some(&"ko" | &"en"))
        || fields.get("style") != Some(&"smartmd")
    {
        return Err(invalid(
            "frontmatter needs title, locale ko/en and style: smartmd",
        ));
    }

    let mut headings = [false; 4];
    let mut blocks = [0; 4];
    let mut prose: [Vec<&str>; 4] = std::array::from_fn(|_| Vec::new());
    let mut section = None;
    let mut callout: Option<(bool, bool)> = None;
    let mut has_callout = false;
    let mut has_table = false;
    let mut i = end + 1;
    while i < lines.len() {
        let line = lines[i];
        if let Some((marker, length, language)) = fence(line) {
            let start = i + 1;
            i = fence_end(&lines, i)?;
            if let Some(which) = LANGUAGES.iter().position(|l| *l == language) {
                if line.starts_with(' ')
                    || lines[i].starts_with(' ')
                    || !line
                        .starts_with(&format!("{}{language}", marker.to_string().repeat(length)))
                {
                    return Err(invalid(
                        "dedicated DSL fences must start at column zero with language immediately after the marker",
                    ));
                }
                if section != Some(which) || callout.is_some() {
                    return Err(invalid(
                        "dedicated DSL must be in its own design section, outside callouts",
                    ));
                }
                validate_dsl(which, &lines[start..i])?;
                blocks[which] += 1;
            }
            i += 1;
            continue;
        }
        // SmartMD consumes balanced attribute blocks before Markdown parsing.
        // Their contents must not masquerade as headings, waivers or fences.
        if line.starts_with('{') {
            let start = i;
            let mut depth = 0_i32;
            let mut quoted = false;
            let mut escaped = false;
            loop {
                for ch in lines[i].chars() {
                    if quoted {
                        if ch == '"' && !escaped {
                            quoted = false;
                        }
                        escaped = ch == '\\' && !escaped;
                    } else if ch == '"' {
                        quoted = true;
                    } else if ch == '{' {
                        depth += 1;
                    } else if ch == '}' {
                        depth -= 1;
                    }
                }
                if depth <= 0 && !quoted {
                    break;
                }
                i += 1;
                if i == lines.len() {
                    return Err(invalid("unclosed SmartMD attribute block"));
                }
            }
            if line.starts_with("{table ") {
                let spec = lines[start..=i].join("\n");
                let after: Vec<_> = lines[i + 1..]
                    .iter()
                    .copied()
                    .skip_while(|l| l.trim().is_empty())
                    .take(3)
                    .collect();
                if column_count(&spec) == after.first().map(|row| table_row(row).len())
                    && after.len() >= 3
                    && table_row(after[0]).len() >= 2
                    && table_row(after[1]).len() == table_row(after[0]).len()
                    && table_row(after[1]).iter().all(|s| {
                        s.trim_matches(':').chars().all(|c| c == '-')
                            && s.trim_matches(':').len() >= 3
                    })
                    && table_row(after[2]).len() == table_row(after[0]).len()
                {
                    has_table = true;
                } else {
                    return Err(invalid(
                        "SmartMD table needs column specifications and its following GFM table",
                    ));
                }
            }
            i += 1;
            continue;
        }
        if line == ":::" {
            if let Some((supported, content)) = callout.take() {
                has_callout |= supported && content;
            } else {
                return Err(invalid("unexpected callout closing line"));
            }
        } else if let Some(rest) = line.strip_prefix(":::") {
            if callout.is_some() {
                return Err(invalid("nested callout"));
            }
            let kind = rest.split_whitespace().next().unwrap_or("");
            callout = Some((
                ["info", "warning", "note", "tip", "success", "danger"].contains(&kind),
                false,
            ));
        } else if let Some((_, content)) = callout.as_mut() {
            *content |= !line.trim().is_empty();
        } else {
            if let Some(name) = line.strip_prefix("## ") {
                section = SECTIONS.iter().position(|s| *s == name.trim());
                if let Some(s) = section {
                    headings[s] = true;
                }
            } else if line.starts_with("# ") {
                section = None;
            }
            if let Some(s) = section {
                prose[s].push(line);
            }
        }
        i += 1;
    }
    if callout.is_some() {
        return Err(invalid("unclosed callout"));
    }
    if !has_callout {
        return Err(invalid("nonempty SmartMD callout required"));
    }
    if !has_table {
        return Err(invalid("SmartMD table specification and table required"));
    }
    for s in 0..4 {
        if !headings[s] {
            return Err(invalid(&format!("missing section: {}", SECTIONS[s])));
        }
        if blocks[s] == 0 && !(matches!(s, 0 | 3) && waiver(&prose[s])) {
            return Err(invalid(&format!(
                "{} needs its {} DSL or a supported applicability explanation",
                SECTIONS[s], LANGUAGES[s]
            )));
        }
        if blocks[s] > 0 && prose[s].contains(&"Applicability: not_applicable") {
            return Err(invalid(
                "not_applicable conflicts with a dedicated DSL in that section",
            ));
        }
    }
    Ok(())
}

fn without_comments(text: &str) -> Result<String> {
    let mut out = String::new();
    let lines: Vec<_> = text.split_inclusive('\n').collect();
    let mut comment = false;
    let mut i = 0;
    while i < lines.len() {
        if !comment && fence(lines[i]).is_some() {
            let end = fence_end(&lines, i)?;
            for line in &lines[i..=end] {
                out.push_str(line);
            }
            i = end + 1;
            continue;
        }
        let mut rest = lines[i];
        loop {
            if comment {
                if let Some((_, after)) = rest.split_once("-->") {
                    rest = after;
                    comment = false;
                } else {
                    if rest.ends_with('\n') {
                        out.push('\n');
                    }
                    break;
                }
            } else if let Some((before, after)) = rest.split_once("<!--") {
                out.push_str(before);
                rest = after;
                comment = true;
            } else {
                out.push_str(rest);
                break;
            }
        }
        i += 1;
    }
    if comment {
        return Err(invalid("unclosed HTML comment"));
    }
    Ok(out)
}

// mdview intentionally treats text fences as containers of nested code examples.
// Match that boundary so example headings and DSL cannot become real sections.
fn fence_end(lines: &[&str], start: usize) -> Result<usize> {
    let (marker, length, language) = fence(lines[start]).unwrap();
    let nested_text = !lines[start].starts_with(' ') && language.eq_ignore_ascii_case("text");
    let mut depth = 0;
    for (index, line) in lines.iter().enumerate().skip(start + 1) {
        if nested_text {
            if line.starts_with(' ') {
                continue;
            }
            if let Some((_, _, info)) = fence(line) {
                if !info.is_empty() {
                    depth += 1;
                } else if depth > 0 {
                    depth -= 1;
                } else {
                    return Ok(index);
                }
            }
        } else if closes_fence(line, marker, length) {
            return Ok(index);
        }
    }
    Err(invalid("unclosed code fence"))
}

fn unquoted_find(text: &str, target: &str) -> Option<usize> {
    let mut quoted = false;
    let mut escaped = false;
    for (i, ch) in text.char_indices() {
        if ch == '"' && !escaped {
            quoted = !quoted;
        }
        if !quoted && text[i..].starts_with(target) {
            return Some(i);
        }
        escaped = quoted && ch == '\\' && !escaped;
    }
    None
}

fn column_count(spec: &str) -> Option<usize> {
    let pos = unquoted_find(spec, "columns")?;
    let rest = spec[pos + "columns".len()..]
        .trim_start()
        .strip_prefix('=')?
        .trim_start()
        .strip_prefix('[')?;
    let end = unquoted_find(rest, "]")?;
    let mut rest = rest[..end].trim();
    let mut count = 0;
    while !rest.is_empty() {
        rest = rest.strip_prefix('{')?;
        let end = unquoted_find(rest, "}")?;
        let mut fields = rest[..end].trim();
        let mut title = false;
        let mut align = false;
        while !fields.is_empty() {
            let end = unquoted_find(fields, ",").unwrap_or(fields.len());
            let (key, value) = fields[..end].split_once('=')?;
            let value = value.trim();
            match key.trim() {
                "title" => {
                    title = value.starts_with('"') && value.ends_with('"') && value.len() > 2
                }
                "align" => align = ["left", "right", "center"].contains(&value),
                "width" if value.trim_end_matches('%').parse::<f64>().is_err() => return None,
                _ => {}
            }
            fields = fields[end..].trim_start_matches(',').trim();
        }
        if !title || !align {
            return None;
        }
        count += 1;
        rest = rest[end + 1..].trim();
        if !rest.is_empty() {
            rest = rest.strip_prefix(',')?.trim();
        }
    }
    (count > 0).then_some(count)
}

fn fence(line: &str) -> Option<(char, usize, &str)> {
    let indent = line.len() - line.trim_start_matches(' ').len();
    if indent > 3 {
        return None;
    }
    let line = &line[indent..];
    let marker = line.chars().next()?;
    if marker != '`' && marker != '~' {
        return None;
    }
    let n = line.chars().take_while(|c| *c == marker).count();
    (n >= 3).then(|| (marker, n, line[n..].trim()))
}

fn closes_fence(line: &str, marker: char, length: usize) -> bool {
    fence(line).is_some_and(|(m, n, info)| m == marker && n >= length && info.is_empty())
}

fn table_row(line: &str) -> Vec<&str> {
    let line = line.trim();
    if !line.starts_with('|') || !line.ends_with('|') {
        return vec![];
    }
    line[1..line.len() - 1].split('|').map(str::trim).collect()
}

fn waiver(lines: &[&str]) -> bool {
    lines.contains(&"Applicability: not_applicable")
        && ["Reason:", "Alternative interface:"].iter().all(|key| {
            lines.iter().any(|line| {
                line.strip_prefix(key).is_some_and(|value| {
                    let value = value.trim();
                    value.chars().count() >= 12
                        && !["unknown", "n/a", "미정", "모름", "해당 없음"]
                            .contains(&value.to_lowercase().as_str())
                })
            })
        })
}

fn validate_dsl(which: usize, lines: &[&str]) -> Result<()> {
    let lines: Vec<_> = lines
        .iter()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect();
    if lines.is_empty() {
        return Err(invalid("empty dedicated DSL"));
    }
    if which == 3 {
        return validate_layout(&lines);
    }
    let mut edges = 0;
    let mut objects = HashSet::new();
    let mut declared = HashSet::new();
    let mut header = false;
    let mut orchestrator = None;
    let mut has_state = false;
    for line in lines {
        if which == 1 {
            if let Some(value) = line
                .strip_prefix("orchestrator:")
                .or_else(|| line.strip_prefix("scope:"))
            {
                if header || value.trim().is_empty() {
                    return Err(invalid("invalid jobflow owner/scope"));
                }
                header = true;
                if line.starts_with("orchestrator:") {
                    orchestrator = Some(value.trim());
                }
                continue;
            }
            if let Some(value) = line.strip_prefix("Object:") {
                for obj in value.split(',').map(str::trim) {
                    if obj.is_empty() || obj.contains('.') {
                        return Err(invalid("invalid jobflow Object declaration"));
                    }
                    declared.insert(obj);
                }
                continue;
            }
        }
        let Some((from, to)) = line.split_once("-->") else {
            return Err(invalid(
                "DSL needs relation lines, not Mermaid or arbitrary tokens",
            ));
        };
        if to.contains("-->") {
            return Err(invalid("one DSL relation per line required"));
        }
        let to = if which == 1 {
            to.trim()
        } else {
            to.split_once(" : ").map_or(to, |(node, _)| node).trim()
        };
        let from = from.trim();
        if from.is_empty() || to.is_empty() {
            return Err(invalid("empty DSL relation endpoint"));
        }
        if which == 1 {
            for endpoint in [from, to] {
                let Some((object, action)) = endpoint.split_once('.') else {
                    return Err(invalid("jobflow endpoints need Object.Action"));
                };
                if object.trim().is_empty() || action.trim().is_empty() || endpoint.contains(" : ")
                {
                    return Err(invalid("invalid jobflow endpoint or arrow label"));
                }
                objects.insert(object);
            }
        } else {
            for endpoint in [from, to] {
                if endpoint.contains(['[', ']', '{', '}', '|', ';']) {
                    return Err(invalid("use dedicated DSL node syntax, not Mermaid shapes"));
                }
                if (endpoint.starts_with('(') && (!endpoint.ends_with(')') || endpoint == "()"))
                    || (endpoint.starts_with('<') && (!endpoint.ends_with('>') || endpoint == "<>"))
                    || (endpoint.starts_with('`') && (!endpoint.ends_with('`') || endpoint == "``"))
                {
                    return Err(invalid("unbalanced or empty DSL node"));
                }
                if which == 0 && endpoint.starts_with('<') {
                    return Err(invalid(
                        "navigation supports pages, processes, API and data nodes",
                    ));
                }
                has_state |= endpoint.starts_with('(') && endpoint.ends_with(')');
            }
        }
        edges += 1;
    }
    if edges == 0 {
        return Err(invalid("dedicated DSL needs at least one relation"));
    }
    if which == 1
        && (!header
            || declared.is_empty()
            || !objects.is_subset(&declared)
            || orchestrator.is_some_and(|owner| !declared.contains(owner)))
    {
        return Err(invalid(
            "jobflow needs owner/scope and all objects explicitly declared",
        ));
    }
    if which == 2 && !has_state {
        return Err(invalid("state DSL needs a parenthesized owned state"));
    }
    Ok(())
}

fn validate_layout(lines: &[&str]) -> Result<()> {
    let mut containers = HashMap::new();
    let mut parents = HashMap::new();
    let mut root = "";
    for line in lines {
        let Some((parent, children)) = line.split_once(" V ").or_else(|| line.split_once(" > "))
        else {
            return Err(invalid("layout requires V or > direction"));
        };
        let parent = parent.trim();
        if parent.is_empty() || containers.contains_key(parent) {
            return Err(invalid("empty/duplicate layout container"));
        }
        if root.is_empty() {
            root = parent;
        }
        let mut names = Vec::new();
        for child in children.split(',') {
            let (name, size) = child
                .trim()
                .split_once(':')
                .map_or((child.trim(), None), |(name, size)| {
                    (name.trim(), Some(size.trim()))
                });
            if name.is_empty()
                || name == parent
                || parents.insert(name, parent).is_some()
                || size.is_some_and(|s| s.parse::<u32>().is_err())
            {
                return Err(invalid(
                    "layout needs unique children, no cycles, and numeric optional sizes without %",
                ));
            }
            names.push(name);
        }
        containers.insert(parent, names);
    }
    if parents.contains_key(root)
        || containers
            .keys()
            .any(|key| *key != root && !parents.contains_key(key))
    {
        return Err(invalid("layout needs one root and a connected tree"));
    }
    for node in containers.keys() {
        let mut seen = HashSet::new();
        let mut current = *node;
        while let Some(parent) = parents.get(current) {
            if !seen.insert(current) {
                bail!("architecture document format: layout cycle");
            }
            current = parent;
        }
        if current != root {
            return Err(invalid("layout has a disconnected tree"));
        }
    }
    Ok(())
}
