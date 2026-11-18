//! Ordered, validated checkpoints for the unchanged complete design package.
use crate::design_package::{self, DesignDocument, DesignPackage, DocumentAnalysis, DocumentKind};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

pub const VERSION: &str = "architalk-design-stages-v1";
pub const RULES: &str = include_str!("../prompts/design-single-document.md");

/// Separate from the nine companion kinds so legacy package schemas stay strict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GenerationKind {
    Drivers,
    QualityScenarios,
    Alternatives,
    Decisions,
    Delivery,
    Jobflow,
    State,
    ScreenLayout,
    Navigation,
    Architecture,
}

impl GenerationKind {
    pub const ALL: [Self; 10] = [
        Self::Drivers,
        Self::QualityScenarios,
        Self::Alternatives,
        Self::Decisions,
        Self::Delivery,
        Self::Jobflow,
        Self::State,
        Self::ScreenLayout,
        Self::Navigation,
        Self::Architecture,
    ];
    pub const ORDER: [Self; 10] = Self::ALL;

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Drivers => "drivers",
            Self::QualityScenarios => "quality_scenarios",
            Self::Alternatives => "alternatives",
            Self::Decisions => "decisions",
            Self::Delivery => "delivery",
            Self::Jobflow => "jobflow",
            Self::State => "state",
            Self::ScreenLayout => "screen_layout",
            Self::Navigation => "navigation",
            Self::Architecture => "architecture",
        }
    }

    pub fn document_kind(self) -> Option<DocumentKind> {
        Some(match self {
            Self::Drivers => DocumentKind::Drivers,
            Self::QualityScenarios => DocumentKind::QualityScenarios,
            Self::Alternatives => DocumentKind::Alternatives,
            Self::Decisions => DocumentKind::Decisions,
            Self::Delivery => DocumentKind::Delivery,
            Self::Jobflow => DocumentKind::Jobflow,
            Self::State => DocumentKind::State,
            Self::ScreenLayout => DocumentKind::ScreenLayout,
            Self::Navigation => DocumentKind::Navigation,
            Self::Architecture => return None,
        })
    }

    pub fn title(self) -> &'static str {
        self.document_kind()
            .map_or("아키텍처 설계", DocumentKind::title)
    }

    pub fn filename(self) -> &'static str {
        self.document_kind()
            .map_or(design_package::MAIN_FILENAME, DocumentKind::filename)
    }
}

impl From<DocumentKind> for GenerationKind {
    fn from(kind: DocumentKind) -> Self {
        Self::ALL
            .into_iter()
            .find(|candidate| candidate.document_kind() == Some(kind))
            .expect("every companion kind has a generation target")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeneratedDocument {
    pub kind: GenerationKind,
    pub markdown: String,
}

fn invalid(detail: impl std::fmt::Display) -> anyhow::Error {
    anyhow::anyhow!("architecture document generation: {detail}")
}

fn trace_owner(id: &str) -> GenerationKind {
    let prefix = id.split_once('-').expect("validated trace ID").0;
    GenerationKind::ALL
        .into_iter()
        .find(|kind| kind.document_kind().and_then(DocumentKind::trace_prefix) == Some(prefix))
        .expect("validated trace prefix has an owner")
}

fn analyze_prefix(completed: &[GeneratedDocument]) -> Result<Vec<DocumentAnalysis>> {
    if completed.len() > GenerationKind::ALL.len() {
        return Err(invalid("at most ten ordered documents may be checkpointed"));
    }
    let mut analyses = Vec::with_capacity(completed.len());
    let mut total = 0;
    for (index, document) in completed.iter().enumerate() {
        if document.kind != GenerationKind::ALL[index] {
            return Err(invalid(format!(
                "document {} must be {}",
                index + 1,
                GenerationKind::ALL[index].as_str()
            )));
        }
        total += document.markdown.len();
        if total > design_package::MAX_PACKAGE_BYTES {
            return Err(invalid("complete package exceeds 500000 UTF-8 bytes"));
        }
        analyses.push(design_package::analyze_document(
            document.kind.document_kind(),
            &document.markdown,
        )?);
    }
    let definitions: BTreeSet<_> = analyses
        .iter()
        .flat_map(|analysis| analysis.definitions.keys().map(String::as_str))
        .collect();
    for (document_index, (document, analysis)) in completed.iter().zip(&analyses).enumerate() {
        for id in &analysis.references {
            if !definitions.contains(id.as_str())
                && completed.iter().any(|item| item.kind == trace_owner(id))
            {
                return Err(invalid(format!(
                    "{}: undefined trace reference after its owner completed: {id}",
                    document.kind.filename()
                )));
            }
        }
        for target in &analysis.links {
            if target.starts_with("https://")
                || target.starts_with("http://")
                || target.starts_with("mailto:")
            {
                continue;
            }
            let (filename, anchor) = target
                .split_once('#')
                .map_or((target.as_str(), None), |(file, anchor)| {
                    (file, Some(anchor))
                });
            let filename = if filename.is_empty() {
                document.kind.filename()
            } else {
                filename
            };
            let Some(target_kind) = GenerationKind::ALL
                .into_iter()
                .find(|kind| kind.filename() == filename)
            else {
                return Err(invalid(format!(
                    "{}: link is not a server-owned package filename",
                    document.kind.filename()
                )));
            };
            if let Some(anchor) = anchor {
                let Some(index) = completed.iter().position(|item| item.kind == target_kind) else {
                    return Err(invalid(format!(
                        "{}: future document links must omit heading anchors: {filename}",
                        document.kind.filename()
                    )));
                };
                if index > document_index {
                    return Err(invalid(format!(
                        "{}: future document links must omit heading anchors: {filename}",
                        document.kind.filename()
                    )));
                }
                if anchor.is_empty() || !analyses[index].anchors.contains(anchor) {
                    return Err(invalid(format!(
                        "{}: unknown heading anchor in {filename}",
                        document.kind.filename()
                    )));
                }
            }
        }
        if document.kind == GenerationKind::Navigation
            && !analysis
                .lines
                .iter()
                .any(|line| line == "Applicability: not_applicable")
            && !analysis.links.iter().any(|target| {
                target.starts_with(&format!("{}#", DocumentKind::ScreenLayout.filename()))
            })
        {
            return Err(invalid(
                "Navigation needs screen verification links to Screen Layout heading anchors",
            ));
        }
    }
    Ok(analyses)
}

/// Revalidate restored checkpoints, including all obligations whose owner exists.
pub fn validate_prefix(completed: &[GeneratedDocument]) -> Result<()> {
    analyze_prefix(completed)?;
    if completed.len() == GenerationKind::ALL.len() {
        design_package::validate(package_from_complete(completed))?;
    }
    Ok(())
}

/// Accept exactly the next requested document, never an aggregate response.
pub fn validate_response(
    text: &str,
    expected: GenerationKind,
    completed: &[GeneratedDocument],
) -> Result<GeneratedDocument> {
    validate_prefix(completed)?;
    if GenerationKind::ALL.get(completed.len()) != Some(&expected) {
        return Err(invalid(
            "requested target is not the next document in the fixed order",
        ));
    }
    if text.len() > design_package::MAX_DOCUMENT_BYTES * 6 + 10_000 {
        return Err(invalid(
            "single document response exceeds the JSON byte limit",
        ));
    }
    let mut document: GeneratedDocument = serde_json::from_str(text.trim())
        .context("AI response must match the single document {kind,markdown} JSON schema")?;
    if document.kind != expected {
        return Err(invalid(format!(
            "response kind must be {}",
            expected.as_str()
        )));
    }
    document.markdown = document.markdown.replace("\r\n", "\n");
    let mut candidate = completed.to_vec();
    candidate.push(document.clone());
    // At the final target this also runs the original whole-package validator.
    validate_prefix(&candidate)?;
    Ok(document)
}

fn package_from_complete(completed: &[GeneratedDocument]) -> DesignPackage {
    DesignPackage {
        markdown: completed
            .last()
            .expect("ten validated documents")
            .markdown
            .clone(),
        documents: completed[..9]
            .iter()
            .map(|document| DesignDocument {
                kind: document.kind.document_kind().expect("companion target"),
                markdown: document.markdown.clone(),
            })
            .collect(),
    }
}

/// Whole-package validation remains the final authority before creating a Draft.
pub fn assemble(completed: &[GeneratedDocument]) -> Result<DesignPackage> {
    if completed.len() != GenerationKind::ALL.len() {
        bail!("architecture document generation: exactly ten documents are required to assemble");
    }
    analyze_prefix(completed)?;
    design_package::validate(package_from_complete(completed))
}

/// Actual extracted symbols, never a model-generated summary or ID rewrite.
pub(crate) fn manifest(completed: &[GeneratedDocument]) -> Result<Value> {
    let analyses = analyze_prefix(completed)?;
    let definitions: BTreeSet<_> = analyses
        .iter()
        .flat_map(|analysis| analysis.definitions.keys().cloned())
        .collect();
    let mut pending: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for (document, analysis) in completed.iter().zip(&analyses) {
        for reference in &analysis.references {
            if !definitions.contains(reference) {
                pending
                    .entry(reference)
                    .or_default()
                    .insert(document.kind.filename());
            }
        }
    }
    Ok(json!({
        "documents": completed.iter().zip(&analyses).map(|(document, analysis)| json!({
            "kind": document.kind,
            "filename": document.kind.filename(),
            "trace_definitions": analysis.definitions.keys().collect::<BTreeSet<_>>(),
            "heading_anchors": analysis.anchors.iter().collect::<BTreeSet<_>>(),
        })).collect::<Vec<_>>(),
        "pending_trace_definitions": pending.into_iter().map(|(id, referenced_by)| {
            let kind = trace_owner(id);
            json!({"id":id,"kind":kind,"filename":kind.filename(),"referenced_by":referenced_by})
        }).collect::<Vec<_>>(),
    }))
}

/// Checkpoints cannot cross prompt/rule/order versions. Runtime settings are not
/// part of this contract, allowing an explicit resume with new model preferences.
pub fn contract_hash() -> String {
    crate::model::hash(
        &serde_json::to_string(&json!({
            "version": VERSION,
            "order": GenerationKind::ALL,
            "package_rules": design_package::RULES,
            "single_document_rules": RULES,
            "max_document_bytes": design_package::MAX_DOCUMENT_BYTES,
            "max_package_bytes": design_package::MAX_PACKAGE_BYTES,
        }))
        .expect("serializable generation contract"),
    )
}
