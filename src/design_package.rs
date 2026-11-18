//! Typed design artifacts. Model output chooses content, never filesystem paths.
use serde::{Deserialize, Serialize};

pub const RULES: &str = include_str!("../prompts/design-package-format.md");
pub const MAIN_FILENAME: &str = "아키텍처-설계.md";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentKind {
    Drivers,
    QualityScenarios,
    Alternatives,
    Decisions,
    Delivery,
    Navigation,
    Jobflow,
    State,
    ScreenLayout,
}

impl DocumentKind {
    pub const ALL: [Self; 9] = [
        Self::Drivers,
        Self::QualityScenarios,
        Self::Alternatives,
        Self::Decisions,
        Self::Delivery,
        Self::Navigation,
        Self::Jobflow,
        Self::State,
        Self::ScreenLayout,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Self::Drivers => "요구사항과 아키텍처 드라이버",
            Self::QualityScenarios => "품질속성과 검증 시나리오",
            Self::Alternatives => "설계 대안과 트레이드오프",
            Self::Decisions => "아키텍처 결정 기록",
            Self::Delivery => "구현과 운영 검증",
            Self::Navigation => "네비게이션",
            Self::Jobflow => "잡플로우",
            Self::State => "상태도",
            Self::ScreenLayout => "화면설계",
        }
    }

    pub fn filename(self) -> &'static str {
        match self {
            Self::Drivers => "요구사항과-아키텍처-드라이버.md",
            Self::QualityScenarios => "품질속성과-검증시나리오.md",
            Self::Alternatives => "설계대안과-트레이드오프.md",
            Self::Decisions => "아키텍처-결정기록.md",
            Self::Delivery => "구현과-운영검증.md",
            Self::Navigation => "네비게이션.md",
            Self::Jobflow => "잡플로우.md",
            Self::State => "상태도.md",
            Self::ScreenLayout => "화면설계.md",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DesignDocument {
    pub kind: DocumentKind,
    pub markdown: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DesignPackage {
    pub markdown: String,
    pub documents: Vec<DesignDocument>,
}

/// Preserve the legacy one-document hash and cover every new artifact's content.
pub fn package_hash(markdown: &str, documents: &[DesignDocument]) -> String {
    if documents.is_empty() {
        return crate::model::hash(markdown);
    }
    let mut ordered: Vec<_> = documents.iter().collect();
    ordered.sort_by_key(|document| document.kind);
    crate::model::hash(
        &serde_json::to_string(&(markdown, ordered)).expect("serializable document package"),
    )
}

pub const MAX_DOCUMENT_BYTES: usize = 150_000;
pub const MAX_PACKAGE_BYTES: usize = 500_000;

impl DocumentKind {
    pub fn required_sections(self) -> &'static [&'static str] {
        match self {
            Self::Drivers => &[
                "Business goals",
                "Requirements and constraints",
                "Architectural drivers",
                "Scope and exclusions",
                "Evidence and open questions",
            ],
            Self::QualityScenarios => &[
                "Quality scenarios",
                "Priority and acceptance",
                "Measurement and unknowns",
            ],
            Self::Alternatives => &[
                "Style and boundaries",
                "Tactics and patterns",
                "Alternative comparison",
                "Trade-offs and rejection",
                "Validation before choice",
            ],
            Self::Decisions => &[
                "Decision records",
                "Decision dependencies",
                "Revisit conditions",
            ],
            Self::Delivery => &[
                "Implementation sequence",
                "Verification matrix",
                "Operations and recovery",
                "Open decisions",
            ],
            Self::Navigation => &["Navigation"],
            Self::Jobflow => &["Jobflow"],
            Self::State => &["State"],
            Self::ScreenLayout => &["Screen Layout"],
        }
    }

    pub(crate) fn trace_prefix(self) -> Option<&'static str> {
        match self {
            Self::Drivers => Some("DRV"),
            Self::QualityScenarios => Some("QA"),
            Self::Alternatives => Some("ALT"),
            Self::Decisions => Some("ADR"),
            Self::Delivery => Some("VAL"),
            _ => None,
        }
    }
}

const PRINCIPAL_SECTIONS: &[&str] = &[
    "Context and scope",
    "Method-R macro design",
    "Method-R system design",
    "Method-R module design",
    "Method-R detailed design",
    "Data and contracts",
    "Runtime and deployment",
    "Document map",
    "Open issues",
];
const TRACE_PREFIXES: [&str; 5] = ["DRV", "QA", "ALT", "ADR", "VAL"];

fn invalid(detail: impl std::fmt::Display) -> anyhow::Error {
    anyhow::anyhow!("architecture design package: {detail}")
}

pub(crate) struct DocumentAnalysis {
    pub lines: Vec<String>,
    pub definitions: std::collections::HashMap<String, Vec<String>>,
    pub references: Vec<String>,
    pub anchors: std::collections::HashSet<String>,
    pub links: Vec<String>,
}

/// The unchanged per-document contract shared by complete and staged generation.
pub(crate) fn analyze_document(
    kind: Option<DocumentKind>,
    markdown: &str,
) -> anyhow::Result<DocumentAnalysis> {
    use crate::document_format::{DiagramKind, ValidationProfile, validate_profile};
    let filename = kind.map_or(MAIN_FILENAME, DocumentKind::filename);
    if markdown.trim().len() < 200 || markdown.len() > MAX_DOCUMENT_BYTES {
        return Err(invalid(format!(
            "{filename}: document must contain 200–150000 UTF-8 bytes"
        )));
    }
    let profile = match kind {
        None => ValidationProfile::Principal,
        Some(DocumentKind::Navigation) => ValidationProfile::Diagram(DiagramKind::Navigation),
        Some(DocumentKind::Jobflow) => ValidationProfile::Diagram(DiagramKind::Jobflow),
        Some(DocumentKind::State) => ValidationProfile::Diagram(DiagramKind::State),
        Some(DocumentKind::ScreenLayout) => ValidationProfile::Diagram(DiagramKind::Layout),
        _ => ValidationProfile::Supporting,
    };
    validate_profile(markdown, profile).map_err(|error| invalid(format!("{filename}: {error}")))?;
    let lines = crate::document_format::outline_lines(markdown)?;
    let sections = kind.map_or(PRINCIPAL_SECTIONS, DocumentKind::required_sections);
    for section in sections {
        let heading = format!("## {section}");
        let Some(start) = lines.iter().position(|line| line == &heading) else {
            return Err(invalid(format!("{filename}: missing section: {section}")));
        };
        let content = lines[start + 1..]
            .iter()
            .take_while(|line| !line.starts_with("## "));
        if !content
            .into_iter()
            .any(|line| !line.trim().is_empty() && !line.starts_with('#'))
        {
            return Err(invalid(format!("{filename}: empty section: {section}")));
        }
    }
    let mut definitions = std::collections::HashMap::new();
    let mut own_definitions = 0;
    for (index, line) in lines.iter().enumerate() {
        let Some(title) = line.strip_prefix("### ") else {
            continue;
        };
        let ids = trace_ids(title)?;
        let Some(id) = ids.first().filter(|id| title.starts_with(id.as_str())) else {
            continue;
        };
        let prefix = id.split_once('-').unwrap().0;
        if kind.and_then(DocumentKind::trace_prefix) != Some(prefix) {
            return Err(invalid(format!(
                "{filename}: {id} must be defined in its owning document"
            )));
        }
        let body: Vec<_> = lines[index + 1..]
            .iter()
            .take_while(|line| !line.starts_with("## ") && !line.starts_with("### "))
            .cloned()
            .collect();
        if definitions.insert(id.clone(), body).is_some() {
            return Err(invalid(format!("duplicate trace definition: {id}")));
        }
        own_definitions += 1;
    }
    if kind.and_then(DocumentKind::trace_prefix).is_some() && own_definitions == 0 {
        return Err(invalid(format!(
            "{filename}: real level-three trace definitions required"
        )));
    }
    let references = trace_ids(&lines.join("\n"))?;
    for (id, body) in &definitions {
        let references = trace_ids(&body.join("\n"))?;
        for prefix in required_trace_prefixes(id) {
            if !references
                .iter()
                .any(|value| value.starts_with(&format!("{prefix}-")))
            {
                return Err(invalid(format!(
                    "{filename}: {id} needs a {prefix} reference in its record"
                )));
            }
        }
    }
    let anchors = heading_anchors(&lines);
    let links = markdown_targets(&lines.join("\n"));
    Ok(DocumentAnalysis {
        lines,
        definitions,
        references,
        anchors,
        links,
    })
}

fn required_trace_prefixes(id: &str) -> &'static [&'static str] {
    match id.split_once('-').unwrap().0 {
        "DRV" => &["QA", "ADR"],
        "QA" => &["DRV"],
        "ALT" => &["DRV", "QA"],
        "ADR" => &["DRV", "QA", "ALT"],
        "VAL" => &["QA", "ADR"],
        _ => unreachable!(),
    }
}

/// Structural checks for newly generated packages. Semantic quality remains a
/// separate independent review of the complete, unabridged package.
pub fn validate(mut package: DesignPackage) -> anyhow::Result<DesignPackage> {
    use std::collections::{HashMap, HashSet};

    if package.documents.len() != DocumentKind::ALL.len() {
        return Err(invalid("exactly nine companion documents are required"));
    }
    let mut seen = HashSet::new();
    for document in &package.documents {
        if !seen.insert(document.kind) {
            return Err(invalid(format!(
                "duplicate document kind: {:?}",
                document.kind
            )));
        }
    }
    package.markdown = package.markdown.replace("\r\n", "\n");
    for document in &mut package.documents {
        document.markdown = document.markdown.replace("\r\n", "\n");
    }
    let total: usize = std::iter::once(&package.markdown)
        .chain(package.documents.iter().map(|d| &d.markdown))
        .map(String::len)
        .sum();
    if total > MAX_PACKAGE_BYTES {
        return Err(invalid("complete package exceeds 500000 UTF-8 bytes"));
    }
    let mut outlines = HashMap::new();
    let mut definitions: HashMap<String, (&str, Vec<String>)> = HashMap::new();
    for (kind, filename, markdown) in
        std::iter::once((None, MAIN_FILENAME, package.markdown.as_str())).chain(
            package
                .documents
                .iter()
                .map(|d| (Some(d.kind), d.kind.filename(), d.markdown.as_str())),
        )
    {
        let analysis = analyze_document(kind, markdown)?;
        for (id, body) in analysis.definitions {
            if definitions.insert(id.clone(), (filename, body)).is_some() {
                return Err(invalid(format!("duplicate trace definition: {id}")));
            }
        }
        outlines.insert(filename, analysis.lines);
    }
    for (filename, lines) in &outlines {
        for id in trace_ids(&lines.join("\n"))? {
            if !definitions.contains_key(&id) {
                return Err(invalid(format!(
                    "{filename}: undefined trace reference: {id}"
                )));
            }
        }
    }
    for (id, (filename, body)) in &definitions {
        let required = required_trace_prefixes(id);
        let references = trace_ids(&body.join("\n"))?;
        for prefix in required {
            if !references
                .iter()
                .any(|value| value.starts_with(&format!("{prefix}-")))
            {
                return Err(invalid(format!(
                    "{filename}: {id} needs a {prefix} reference in its record"
                )));
            }
        }
    }
    validate_links(&outlines)?;
    validate_module_collaboration(&package, &outlines)?;
    let map = section_lines(&outlines[MAIN_FILENAME], "Document map");
    let main_links = markdown_targets(&map.join("\n"));
    for kind in DocumentKind::ALL {
        if !main_links
            .iter()
            .any(|target| target.split('#').next() == Some(kind.filename()))
        {
            return Err(invalid(format!(
                "Document map must link {}",
                kind.filename()
            )));
        }
    }
    let navigation = &outlines[DocumentKind::Navigation.filename()];
    if !navigation
        .iter()
        .any(|line| line == "Applicability: not_applicable")
        && !markdown_targets(&navigation.join("\n"))
            .iter()
            .any(|target| {
                target.starts_with(&format!("{}#", DocumentKind::ScreenLayout.filename()))
            })
    {
        return Err(invalid(
            "Navigation needs screen verification links to Screen Layout heading anchors",
        ));
    }
    Ok(package)
}

fn section_lines<'a>(lines: &'a [String], section: &str) -> &'a [String] {
    let marker = format!("## {section}");
    let Some(start) = lines.iter().position(|line| line == &marker) else {
        return &[];
    };
    let end = lines[start + 1..]
        .iter()
        .position(|line| line.starts_with("## "))
        .map_or(lines.len(), |offset| start + 1 + offset);
    &lines[start + 1..end]
}

fn trace_ids(text: &str) -> anyhow::Result<Vec<String>> {
    let mut ids = Vec::new();
    for (start, _) in text.char_indices() {
        if start > 0
            && text[..start]
                .chars()
                .next_back()
                .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            continue;
        }
        let rest = &text[start..];
        for prefix in TRACE_PREFIXES {
            let Some(tail) = rest.strip_prefix(prefix).and_then(|s| s.strip_prefix('-')) else {
                continue;
            };
            let digits = tail.bytes().take_while(u8::is_ascii_digit).count();
            if digits != 3
                || tail[digits..]
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
            {
                return Err(invalid(format!(
                    "trace IDs require exactly three digits: {prefix}-..."
                )));
            }
            ids.push(format!("{prefix}-{}", &tail[..digits]));
        }
    }
    Ok(ids)
}

/// GitHub/marked-style heading anchors used by the document viewer. Duplicate
/// headings receive the conventional numeric suffix.
fn heading_anchors(lines: &[String]) -> std::collections::HashSet<String> {
    heading_anchor_order(lines).into_iter().collect()
}

fn heading_anchor_order(lines: &[String]) -> Vec<String> {
    let mut anchors = std::collections::HashSet::new();
    let mut ordered = Vec::new();
    for line in lines {
        let Some((_, title)) = crate::document_format::atx_heading(line) else {
            continue;
        };
        use unicode_normalization::UnicodeNormalization;
        let title = heading_text(title.trim_end_matches('#').trim());
        let anchor: String = title
            .nfc()
            .collect::<String>()
            .to_lowercase()
            .chars()
            .filter_map(|c| {
                if c.is_alphanumeric() || c == '_' || c == '-' {
                    Some(c)
                } else if c.is_whitespace() {
                    Some('-')
                } else {
                    None
                }
            })
            .collect();
        let mut candidate = anchor.clone();
        let mut duplicate = 0;
        while anchors.contains(&candidate) {
            duplicate += 1;
            candidate = format!("{anchor}-{duplicate}");
        }
        anchors.insert(candidate.clone());
        ordered.push(candidate);
    }
    ordered
}

fn validate_module_collaboration(
    package: &DesignPackage,
    outlines: &std::collections::HashMap<&str, Vec<String>>,
) -> anyhow::Result<()> {
    let main_scopes = crate::document_format::orchestrator_jobflow_headings(&package.markdown)?;
    if heading_anchor_order(&outlines[MAIN_FILENAME])
        .iter()
        .enumerate()
        .any(|(ordinal, anchor)| {
            anchor == "method-r-module-design" && main_scopes.contains(&ordinal)
        })
    {
        return Ok(());
    }
    let jobflow = package
        .documents
        .iter()
        .find(|document| document.kind == DocumentKind::Jobflow)
        .expect("nine unique typed documents were checked");
    let scopes = crate::document_format::orchestrator_jobflow_headings(&jobflow.markdown)?;
    let anchors = heading_anchor_order(&outlines[jobflow.kind.filename()]);
    let module = section_lines(&outlines[MAIN_FILENAME], "Method-R module design").join("\n");
    // Delegation uses an explicit inline Markdown link, never an example in a
    // code span or an unused reference definition. No path is followed on disk.
    let prose = without_inline_code(&module);
    if module_link_targets(&prose).iter().any(|target| {
        let Some((file, anchor)) = target.split_once('#') else {
            return false;
        };
        file == jobflow.kind.filename()
            && !anchor.is_empty()
            && anchors
                .iter()
                .enumerate()
                .any(|(ordinal, candidate)| candidate == anchor && scopes.contains(&ordinal))
    }) {
        return Ok(());
    }
    Err(invalid(
        "Method-R module design needs its own orchestrator jobflow or an explicit scoped Markdown link to an orchestrator jobflow heading in 잡플로우.md",
    ))
}

// The module-delegation contract intentionally uses a single-line text label
// and a bare/angle destination, without an optional link title. This small
// scanner is not a general Markdown parser; non-links cannot establish proof.
fn module_link_targets(text: &str) -> Vec<String> {
    fn escaped(text: &str, index: usize) -> bool {
        text.as_bytes()[..index]
            .iter()
            .rev()
            .take_while(|byte| **byte == b'\\')
            .count()
            % 2
            == 1
    }
    let mut stack = Vec::new();
    let mut targets = Vec::new();
    for (index, character) in text.char_indices() {
        if escaped(text, index) {
            continue;
        }
        if character == '[' {
            let image =
                index > 0 && text.as_bytes()[index - 1] == b'!' && !escaped(text, index - 1);
            stack.push((index, image));
        } else if character == ']' {
            let Some((start, image)) = stack.pop() else {
                continue;
            };
            let label = &text[start + 1..index];
            if image
                || !stack.is_empty()
                || label.trim().is_empty()
                || label.contains(['[', ']', '\n'])
            {
                continue;
            }
            let Some(after) = text[index + 1..].strip_prefix('(') else {
                continue;
            };
            let Some(end) = after.find(')') else {
                continue;
            };
            let raw = after[..end].trim();
            let target = raw
                .strip_prefix('<')
                .and_then(|value| value.strip_suffix('>'))
                .unwrap_or(raw);
            if target.is_empty()
                || target.chars().any(|character| {
                    character.is_whitespace()
                        || character.is_control()
                        || matches!(character, '\\' | '<' | '>')
                })
            {
                continue;
            }
            targets.push(target.to_owned());
        }
    }
    targets
}

fn without_inline_code(text: &str) -> String {
    let mut output = String::new();
    let mut rest = text;
    while let Some(start) = rest.find('`') {
        output.push_str(&rest[..start]);
        let count = rest[start..]
            .bytes()
            .take_while(|byte| *byte == b'`')
            .count();
        let marker = "`".repeat(count);
        let after = &rest[start + count..];
        if let Some(end) = after.find(&marker) {
            output.push(' ');
            rest = &after[end + count..];
        } else {
            output.push_str(&rest[start..]);
            return output;
        }
    }
    output.push_str(rest);
    output
}

fn heading_text(title: &str) -> String {
    let mut plain = String::new();
    let mut rest = title;
    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix('\\') {
            if let Some(character) = after.chars().next() {
                plain.push(character);
                rest = &after[character.len_utf8()..];
                continue;
            }
        }
        if rest.starts_with('`') {
            let count = rest.bytes().take_while(|byte| *byte == b'`').count();
            let marker = "`".repeat(count);
            if let Some(end) = rest[count..].find(&marker) {
                plain.push_str(&rest[count..count + end]);
                rest = &rest[count + end + count..];
                continue;
            }
        }
        if rest.starts_with('_')
            && plain
                .chars()
                .next_back()
                .is_none_or(|c| !c.is_alphanumeric())
        {
            let count = rest.bytes().take_while(|byte| *byte == b'_').count();
            if count <= 3 {
                let marker = "_".repeat(count);
                let after_open = &rest[count..];
                let closing = after_open.match_indices(&marker).find(|(end, _)| {
                    let inside = &after_open[..*end];
                    !inside.is_empty()
                        && inside.trim() == inside
                        && after_open[end + count..]
                            .chars()
                            .next()
                            .is_none_or(|c| !c.is_alphanumeric())
                });
                if let Some((end, _)) = closing {
                    plain.push_str(&after_open[..end]);
                    rest = &after_open[end + count..];
                    continue;
                }
            }
        }
        if rest.starts_with('[') || rest.starts_with("![") {
            let opening = usize::from(rest.starts_with('!'));
            if let Some(end) = rest[opening + 1..].find(']') {
                let end = opening + 1 + end;
                let after = &rest[end + 1..];
                let closing = if after.starts_with('(') {
                    after.find(')')
                } else if after.starts_with('[') {
                    after.find(']')
                } else {
                    None
                };
                if let Some(closing) = closing {
                    plain.push_str(&rest[opening + 1..end]);
                    rest = &after[closing + 1..];
                    continue;
                }
            }
        }
        if let Some(after) = rest.strip_prefix('<') {
            if let Some(end) = after.find('>') {
                let inside = &after[..end];
                if inside
                    .trim_start_matches('/')
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_alphabetic())
                    && !inside.contains("://")
                {
                    rest = &after[end + 1..];
                    continue;
                }
            }
        }
        if let Some(after) = rest.strip_prefix('&') {
            if let Some(end) = after.find(';').filter(|end| *end <= 12) {
                let entity = &after[..end];
                let decoded = match entity {
                    "amp" => Some('&'),
                    "lt" => Some('<'),
                    "gt" => Some('>'),
                    "quot" => Some('"'),
                    "apos" => Some('\''),
                    "nbsp" => Some(' '),
                    _ => entity
                        .strip_prefix("#x")
                        .or_else(|| entity.strip_prefix("#X"))
                        .and_then(|number| u32::from_str_radix(number, 16).ok())
                        .and_then(char::from_u32)
                        .or_else(|| {
                            entity
                                .strip_prefix('#')
                                .and_then(|number| number.parse().ok())
                                .and_then(char::from_u32)
                        }),
                };
                if let Some(character) = decoded {
                    plain.push(character);
                    rest = &after[end + 1..];
                    continue;
                }
            }
        }
        let character = rest.chars().next().unwrap();
        plain.push(character);
        rest = &rest[character.len_utf8()..];
    }
    plain
}

fn inline_markdown_targets(text: &str) -> Vec<String> {
    let mut targets = Vec::new();
    let mut rest = text;
    while let Some(index) = rest.find("](") {
        rest = &rest[index + 2..];
        let Some(end) = rest.find(')') else { break };
        let value = rest[..end].trim();
        let value = if let Some(value) = value.strip_prefix('<') {
            value.split_once('>').map_or(value, |(target, _)| target)
        } else {
            value.split_whitespace().next().unwrap_or("")
        };
        targets.push(value.to_owned());
        rest = &rest[end + 1..];
    }
    targets
}

fn markdown_targets(text: &str) -> Vec<String> {
    let mut targets = inline_markdown_targets(text);
    // Reference-style definitions are validated too; their uses may be elsewhere.
    for line in text.lines() {
        if line.starts_with('[') {
            if let Some((_, target)) = line.split_once("]:") {
                targets.push(
                    target
                        .split_whitespace()
                        .next()
                        .unwrap_or("")
                        .trim_matches(['<', '>'])
                        .to_owned(),
                );
            }
        }
    }
    targets
}

fn validate_links(outlines: &std::collections::HashMap<&str, Vec<String>>) -> anyhow::Result<()> {
    let anchors: std::collections::HashMap<_, _> = outlines
        .iter()
        .map(|(name, lines)| (*name, heading_anchors(lines)))
        .collect();
    for (filename, lines) in outlines {
        for target in markdown_targets(&lines.join("\n")) {
            if target.starts_with("https://")
                || target.starts_with("http://")
                || target.starts_with("mailto:")
            {
                continue;
            }
            let (file, anchor) = target
                .split_once('#')
                .map_or((target.as_str(), None), |(file, anchor)| {
                    (file, Some(anchor))
                });
            let file = if file.is_empty() { *filename } else { file };
            let Some(valid_anchors) = anchors.get(file) else {
                return Err(invalid(format!(
                    "{filename}: link is not a server-owned package filename: {target}"
                )));
            };
            if anchor.is_some_and(|value| value.is_empty() || !valid_anchors.contains(value)) {
                return Err(invalid(format!(
                    "{filename}: unknown heading anchor: {target}"
                )));
            }
        }
    }
    Ok(())
}
