//! Versioned multi-output procedural asset collections.
//!
//! A collection combines shared procedural resources, lightweight generated
//! outputs, and embedded ordinary Spriterize projects. The archive is a zip
//! with a readable JSON manifest so build tools can inspect its organization
//! without running the editor.

use crate::wrapped_image::WrappedImage;
use image::{codecs::png::PngEncoder, ColorType, ImageEncoder};
use lapix::{Bitmap, ExportOptions, GenValue, GeneratorDefinition};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use thiserror::Error;
use zip::write::SimpleFileOptions;

pub const COLLECTION_EXTENSION: &str = "spriterize-collection";
pub const FORMAT_VERSION: u32 = 2;
const MANIFEST_PATH: &str = "manifest.json";

#[derive(Debug, Error)]
pub enum CollectionError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("archive error: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("manifest error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid collection: {0}")]
    Invalid(String),
    #[error("generator `{asset}` failed: {message}")]
    Generate { asset: String, message: String },
    #[error("PNG encoding failed: {0}")]
    Png(#[from] image::ImageError),
}

pub type Result<T> = std::result::Result<T, CollectionError>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssetCollection {
    pub format_version: u32,
    pub name: String,
    #[serde(default)]
    pub script_libraries: Vec<ScriptLibrary>,
    #[serde(default)]
    pub generators: Vec<GeneratorResource>,
    #[serde(default)]
    pub assets: Vec<AssetOutput>,
    /// Fully editable raster projects stored as ordinary project payloads in
    /// archive entries rather than inflated into JSON.
    #[serde(default)]
    pub projects: Vec<CollectionProject>,
}

impl AssetCollection {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            format_version: FORMAT_VERSION,
            name: name.into(),
            script_libraries: Vec::new(),
            generators: Vec::new(),
            assets: Vec::new(),
            projects: Vec::new(),
        }
    }

    pub fn validate(&self) -> Result<()> {
        if !(1..=FORMAT_VERSION).contains(&self.format_version) {
            return Err(CollectionError::Invalid(format!(
                "unsupported format version {} (latest is {FORMAT_VERSION})",
                self.format_version
            )));
        }
        if self.name.trim().is_empty() {
            return Err(CollectionError::Invalid("collection name is empty".into()));
        }

        let libraries = unique_ids(
            self.script_libraries
                .iter()
                .map(|resource| resource.id.as_str()),
            "script library",
        )?;
        let generators = unique_ids(
            self.generators.iter().map(|resource| resource.id.as_str()),
            "generator",
        )?;
        let assets = unique_ids(self.assets.iter().map(|asset| asset.id.as_str()), "asset")?;
        let projects = unique_ids(
            self.projects.iter().map(|project| project.id.as_str()),
            "project",
        )?;
        if let Some(id) = projects.iter().find(|id| assets.contains(**id)) {
            return Err(CollectionError::Invalid(format!(
                "id `{id}` is used by both an asset and a project"
            )));
        }
        if self.format_version < 2 && !self.projects.is_empty() {
            return Err(CollectionError::Invalid(
                "format version 1 cannot contain embedded projects".into(),
            ));
        }

        for generator in &self.generators {
            if matches!(generator.definition, GeneratorDefinition::Graph(_))
                && !generator.libraries.is_empty()
            {
                return Err(CollectionError::Invalid(format!(
                    "graph generator `{}` cannot include script libraries",
                    generator.id
                )));
            }
            let mut used = HashSet::new();
            for library in &generator.libraries {
                if !libraries.contains(library.as_str()) {
                    return Err(CollectionError::Invalid(format!(
                        "generator `{}` references missing script library `{library}`",
                        generator.id
                    )));
                }
                if !used.insert(library) {
                    return Err(CollectionError::Invalid(format!(
                        "generator `{}` includes script library `{library}` more than once",
                        generator.id
                    )));
                }
            }
        }

        let mut output_paths = HashSet::new();
        for asset in &self.assets {
            if asset.width == 0 || asset.height == 0 {
                return Err(CollectionError::Invalid(format!(
                    "asset `{}` has a zero dimension",
                    asset.id
                )));
            }
            if !generators.contains(asset.generator.as_str()) {
                return Err(CollectionError::Invalid(format!(
                    "asset `{}` references missing generator `{}`",
                    asset.id, asset.generator
                )));
            }
            let path = safe_relative_path(&asset.output)?;
            let key = path.to_string_lossy().replace('\\', "/").to_lowercase();
            if !output_paths.insert(key) {
                return Err(CollectionError::Invalid(format!(
                    "more than one asset exports to `{}`",
                    path.display()
                )));
            }
            let mut value_ids = HashSet::new();
            for (id, _) in &asset.values {
                if id.trim().is_empty() || !value_ids.insert(id) {
                    return Err(CollectionError::Invalid(format!(
                        "asset `{}` has an empty or duplicate knob value id",
                        asset.id
                    )));
                }
            }
            if let Some(slice) = asset.metadata.slice9 {
                if slice[0] + slice[2] > asset.width || slice[1] + slice[3] > asset.height {
                    return Err(CollectionError::Invalid(format!(
                        "asset `{}` has slice9 margins larger than its dimensions",
                        asset.id
                    )));
                }
            }
        }
        let mut entries = HashSet::new();
        for project in &self.projects {
            if project.name.trim().is_empty() {
                return Err(CollectionError::Invalid(format!(
                    "project `{}` has an empty name",
                    project.id
                )));
            }
            let entry = safe_relative_path(&project.entry)?;
            if !entry.starts_with("projects") || !entries.insert(project.entry.to_lowercase()) {
                return Err(CollectionError::Invalid(format!(
                    "project `{}` has an invalid or duplicate archive entry `{}`",
                    project.id, project.entry
                )));
            }
            if project.data.is_empty() {
                return Err(CollectionError::Invalid(format!(
                    "project `{}` has no project data",
                    project.id
                )));
            }
            lapix::State::<WrappedImage>::from_project_bytes(&project.data).map_err(|error| {
                CollectionError::Invalid(format!(
                    "project `{}` cannot be decoded: {error}",
                    project.id
                ))
            })?;
            let path = safe_relative_path(&project.output)?;
            let key = path.to_string_lossy().replace('\\', "/").to_lowercase();
            if !output_paths.insert(key) {
                return Err(CollectionError::Invalid(format!(
                    "more than one output exports to `{}`",
                    path.display()
                )));
            }
        }
        Ok(())
    }

    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        self.validate()?;
        let manifest = serde_json::to_vec_pretty(self)?;
        let file = File::create(path)?;
        let mut archive = zip::ZipWriter::new(file);
        let options = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .unix_permissions(0o644);
        archive.start_file(MANIFEST_PATH, options)?;
        archive.write_all(&manifest)?;
        for project in &self.projects {
            archive.start_file(&project.entry, options)?;
            archive.write_all(&crate::project::with_header(project.data.clone()))?;
        }
        archive.finish()?;
        Ok(())
    }

    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let file = File::open(path)?;
        let mut archive = zip::ZipArchive::new(file)?;
        let mut manifest = Vec::new();
        archive.by_name(MANIFEST_PATH)?.read_to_end(&mut manifest)?;
        let mut collection: Self = serde_json::from_slice(&manifest)?;
        for project in &mut collection.projects {
            let mut bytes = Vec::new();
            archive.by_name(&project.entry)?.read_to_end(&mut bytes)?;
            project.data = crate::project::without_header(bytes);
        }
        collection.validate()?;
        Ok(collection)
    }

    /// Add a blank/editor-created project payload and return its stable id.
    pub fn add_project(&mut self, name: impl Into<String>, data: Vec<u8>) -> Result<String> {
        let name = name.into();
        let base = slug(&name);
        let mut id = base.clone();
        let mut suffix = 2;
        while self.assets.iter().any(|asset| asset.id == id)
            || self.projects.iter().any(|project| project.id == id)
        {
            id = format!("{base}-{suffix}");
            suffix += 1;
        }
        let previous_version = self.format_version;
        self.format_version = FORMAT_VERSION;
        self.projects.push(CollectionProject {
            id: id.clone(),
            name,
            entry: format!("projects/{id}.spriterize"),
            output: format!("{id}.png"),
            export: ExportOptions::default(),
            metadata: AssetMetadata::default(),
            data,
        });
        if let Err(error) = self.validate() {
            self.projects.pop();
            self.format_version = previous_version;
            return Err(error);
        }
        Ok(id)
    }

    pub fn export_all(&self, directory: impl AsRef<Path>) -> Result<ExportReport> {
        self.export_selected(directory, std::iter::empty::<&str>())
    }

    /// Export selected asset ids. An empty selection means all assets.
    pub fn export_selected<'a>(
        &self,
        directory: impl AsRef<Path>,
        selected: impl IntoIterator<Item = &'a str>,
    ) -> Result<ExportReport> {
        self.validate()?;
        let selected: HashSet<&str> = selected.into_iter().collect();
        if !selected.is_empty() {
            let known: HashSet<&str> = self
                .assets
                .iter()
                .map(|asset| asset.id.as_str())
                .chain(self.projects.iter().map(|project| project.id.as_str()))
                .collect();
            if let Some(unknown) = selected.iter().find(|id| !known.contains(**id)) {
                return Err(CollectionError::Invalid(format!(
                    "selected asset `{unknown}` does not exist"
                )));
            }
        }

        // Generate everything before touching the destination, so a bad recipe
        // cannot leave a half-exported collection behind.
        let mut generated = Vec::new();
        for asset in self
            .assets
            .iter()
            .filter(|asset| selected.is_empty() || selected.contains(asset.id.as_str()))
        {
            let pixels = self.generate(asset)?;
            let image =
                WrappedImage::from_parts((asset.width as i32, asset.height as i32).into(), &pixels);
            let image = lapix::export::prepare(&image, &asset.export, None);
            generated.push((asset.output.clone(), image));
        }
        for project in self
            .projects
            .iter()
            .filter(|project| selected.is_empty() || selected.contains(project.id.as_str()))
        {
            let state = lapix::State::<WrappedImage>::from_project_bytes(&project.data).map_err(
                |error| CollectionError::Generate {
                    asset: project.id.clone(),
                    message: error.to_string(),
                },
            )?;
            let image = state.composite().clone();
            let image = lapix::export::prepare(&image, &project.export, None);
            generated.push((project.output.clone(), image));
        }

        let root = directory.as_ref();
        std::fs::create_dir_all(root)?;
        let mut paths = Vec::with_capacity(generated.len());
        for (output, image) in generated {
            let relative = png_path(safe_relative_path(&output)?);
            let path = root.join(relative);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let file = File::create(&path)?;
            PngEncoder::new(file).write_image(
                image.bytes(),
                image.width() as u32,
                image.height() as u32,
                ColorType::Rgba8,
            )?;
            paths.push(path);
        }
        Ok(ExportReport { paths })
    }

    pub fn generate(&self, asset: &AssetOutput) -> Result<Vec<u8>> {
        let generator = self
            .generators
            .iter()
            .find(|generator| generator.id == asset.generator)
            .ok_or_else(|| {
                CollectionError::Invalid(format!(
                    "asset `{}` references missing generator `{}`",
                    asset.id, asset.generator
                ))
            })?;
        let failure = |message| CollectionError::Generate {
            asset: asset.id.clone(),
            message,
        };
        match &generator.definition {
            GeneratorDefinition::Script(script) => {
                let library_map: HashMap<&str, &str> = self
                    .script_libraries
                    .iter()
                    .map(|library| (library.id.as_str(), library.source.as_str()))
                    .collect();
                let mut source = String::new();
                for id in &generator.libraries {
                    source.push_str(library_map[id.as_str()]);
                    source.push('\n');
                }
                source.push_str(script);
                artlib_script::generate(
                    &source,
                    asset.width as usize,
                    asset.height as usize,
                    script_values(&asset.values),
                )
                .map(|generated| generated.pixels)
                .map_err(failure)
            }
            GeneratorDefinition::Graph(recipe) => {
                let graph = crate::gui::graph::from_recipe(recipe).map_err(failure)?;
                crate::gui::graph::evaluate_with_values(
                    &graph,
                    asset.width as usize,
                    asset.height as usize,
                    &graph_values(&asset.values),
                )
                .map_err(failure)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScriptLibrary {
    pub id: String,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeneratorResource {
    pub id: String,
    pub definition: GeneratorDefinition,
    /// Ordered helper libraries prepended to a script before compilation.
    #[serde(default)]
    pub libraries: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssetOutput {
    pub id: String,
    pub generator: String,
    pub width: u32,
    pub height: u32,
    pub output: String,
    #[serde(default)]
    pub values: Vec<(String, GenValue)>,
    #[serde(default)]
    pub export: ExportOptions,
    #[serde(default)]
    pub metadata: AssetMetadata,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CollectionProject {
    pub id: String,
    pub name: String,
    /// ZIP entry holding a header-prefixed ordinary Spriterize project.
    pub entry: String,
    pub output: String,
    #[serde(default)]
    pub export: ExportOptions,
    #[serde(default)]
    pub metadata: AssetMetadata,
    #[serde(skip)]
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct AssetMetadata {
    pub slice9: Option<[u32; 4]>,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportReport {
    pub paths: Vec<PathBuf>,
}

fn unique_ids<'a>(ids: impl IntoIterator<Item = &'a str>, kind: &str) -> Result<HashSet<&'a str>> {
    let mut found = HashSet::new();
    for id in ids {
        if id.trim().is_empty() {
            return Err(CollectionError::Invalid(format!("{kind} id is empty")));
        }
        if !found.insert(id) {
            return Err(CollectionError::Invalid(format!(
                "duplicate {kind} id `{id}`"
            )));
        }
    }
    Ok(found)
}

fn slug(name: &str) -> String {
    let value: String = name
        .trim()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let value = value.trim_matches('-');
    if value.is_empty() {
        "project".into()
    } else {
        value.into()
    }
}

fn safe_relative_path(value: &str) -> Result<PathBuf> {
    let path = Path::new(value);
    if value.trim().is_empty()
        || path
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err(CollectionError::Invalid(format!(
            "output path `{value}` is not a safe relative path"
        )));
    }
    Ok(path.to_owned())
}

fn png_path(mut path: PathBuf) -> PathBuf {
    if !path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("png"))
    {
        path.set_extension("png");
    }
    path
}

fn script_values(values: &[(String, GenValue)]) -> artlib_script::KnobValues {
    values
        .iter()
        .map(|(id, value)| {
            let value = match value {
                GenValue::Float(value) => artlib_script::KnobValue::Float(*value as f64),
                GenValue::Int(value) => artlib_script::KnobValue::Int(*value),
                GenValue::Color(value) => {
                    artlib_script::KnobValue::Color([value.r, value.g, value.b, value.a])
                }
                GenValue::Bool(value) => artlib_script::KnobValue::Bool(*value),
            };
            (id.clone(), value)
        })
        .collect()
}

fn graph_values(values: &[(String, GenValue)]) -> crate::gui::graph::KnobValues {
    values
        .iter()
        .map(|(id, value)| {
            let value = match value {
                GenValue::Float(value) => crate::gui::graph::KnobValue::Float(*value),
                GenValue::Int(value) => crate::gui::graph::KnobValue::Int(*value),
                GenValue::Color(value) => {
                    crate::gui::graph::KnobValue::Color([value.r, value.g, value.b, value.a])
                }
                GenValue::Bool(value) => crate::gui::graph::KnobValue::Bool(*value),
            };
            (id.clone(), value)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use lapix::{GeneratorGraph, GeneratorGraphNode, GeneratorGraphWire, GeneratorNode};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("spriterize-{name}-{}-{nonce}", std::process::id()))
    }

    fn script_collection() -> AssetCollection {
        let mut collection = AssetCollection::new("test assets");
        collection.script_libraries.push(ScriptLibrary {
            id: "shared-shapes".into(),
            source: "fn shared_shape(w, h) { disk(w as f64 / 2.0, h as f64 / 2.0, 3.0) }".into(),
        });
        collection.generators.push(GeneratorResource {
            id: "dot".into(),
            definition: GeneratorDefinition::Script(
                "pub fn main(w, h, p) { let c = Canvas::new(w, h); c.paint(shared_shape(w, h), solid(rgb(255, 90, 40))); c }".into(),
            ),
            libraries: vec!["shared-shapes".into()],
        });
        collection.assets.push(AssetOutput {
            id: "dot-small".into(),
            generator: "dot".into(),
            width: 16,
            height: 16,
            output: "ui/dot-small".into(),
            values: Vec::new(),
            export: ExportOptions::default(),
            metadata: AssetMetadata {
                slice9: None,
                tags: vec!["ui".into()],
            },
        });
        collection
    }

    #[test]
    fn archive_round_trip_is_manifest_based() {
        let collection = script_collection();
        let dir = temp_dir("collection-roundtrip");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("assets.{COLLECTION_EXTENSION}"));
        collection.save(&path).unwrap();
        assert_eq!(AssetCollection::load(&path).unwrap(), collection);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn shared_script_resources_export_pngs() {
        let collection = script_collection();
        let dir = temp_dir("collection-export");
        let report = collection.export_all(&dir).unwrap();
        assert_eq!(report.paths, vec![dir.join("ui/dot-small.png")]);
        assert!(report.paths[0].is_file());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn graph_resources_generate_without_projects() {
        let mut collection = AssetCollection::new("graph assets");
        collection.generators.push(GeneratorResource {
            id: "blank".into(),
            definition: GeneratorDefinition::Graph(GeneratorGraph {
                nodes: vec![GeneratorGraphNode {
                    id: 1,
                    position: [0.0, 0.0],
                    node: GeneratorNode::Output,
                }],
                wires: Vec::<GeneratorGraphWire>::new(),
            }),
            libraries: Vec::new(),
        });
        collection.assets.push(AssetOutput {
            id: "blank".into(),
            generator: "blank".into(),
            width: 3,
            height: 2,
            output: "blank.png".into(),
            values: Vec::new(),
            export: ExportOptions::default(),
            metadata: AssetMetadata::default(),
        });
        assert_eq!(
            collection.generate(&collection.assets[0]).unwrap(),
            vec![0; 3 * 2 * 4]
        );
    }

    #[test]
    fn validation_rejects_traversal_and_missing_resources() {
        let mut collection = script_collection();
        collection.assets[0].output = "../outside.png".into();
        assert!(collection.validate().is_err());
        collection.assets[0].output = "inside.png".into();
        collection.assets[0].generator = "missing".into();
        assert!(collection.validate().is_err());
    }

    #[test]
    fn embedded_projects_round_trip_and_export() {
        let state = lapix::State::<WrappedImage>::new((7, 5).into(), None, None);
        let mut collection = AssetCollection::new("editable assets");
        let id = collection
            .add_project("Tiny Hero", state.project_bytes().unwrap())
            .unwrap();
        assert_eq!(id, "tiny-hero");

        let dir = temp_dir("embedded-project");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("assets.{COLLECTION_EXTENSION}"));
        collection.save(&path).unwrap();
        let loaded = AssetCollection::load(&path).unwrap();
        assert_eq!(loaded, collection);

        let export_dir = dir.join("export");
        let report = loaded.export_all(&export_dir).unwrap();
        assert_eq!(report.paths, vec![export_dir.join("tiny-hero.png")]);
        let image = image::open(&report.paths[0]).unwrap();
        assert_eq!((image.width(), image.height()), (7, 5));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn embedded_project_ids_are_stable_and_unique() {
        let state = lapix::State::<WrappedImage>::new((1, 1).into(), None, None);
        let bytes = state.project_bytes().unwrap();
        let mut collection = AssetCollection::new("ids");
        assert_eq!(
            collection.add_project("Magic Orb", bytes.clone()).unwrap(),
            "magic-orb"
        );
        assert_eq!(
            collection.add_project("Magic Orb", bytes).unwrap(),
            "magic-orb-2"
        );
    }

    #[test]
    fn malformed_embedded_projects_are_rejected() {
        let mut collection = AssetCollection::new("broken");
        assert!(collection.add_project("Broken", vec![1, 2, 3, 4]).is_err());
    }
}
