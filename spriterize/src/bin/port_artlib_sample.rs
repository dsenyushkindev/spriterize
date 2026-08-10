use lapix::{Bitmap, Generator, GeneratorGraph, GeneratorNode, State};
use serde::Deserialize;
use spriterize::collection::{
    AssetCollection, AssetMetadata, CollectionProject, ElementResource, COLLECTION_EXTENSION,
    FORMAT_VERSION,
};
use spriterize::wrapped_image::WrappedImage;
use std::path::{Path, PathBuf};

#[derive(Deserialize)]
struct Trace {
    name: String,
    #[serde(default)]
    elements: Vec<ElementResource>,
    assets: Vec<TracedAsset>,
}

#[derive(Deserialize)]
struct TracedAsset {
    id: String,
    name: String,
    width: u32,
    height: u32,
    output: String,
    slice9: Option<[u32; 4]>,
    image: String,
    graph: GeneratorGraph,
    tags: Vec<String>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let trace_path = args
        .next()
        .map(PathBuf::from)
        .ok_or("usage: port_artlib_sample TRACE.json [OUTPUT.spriterize-collection]")?;
    let output = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| trace_path.with_file_name(format!("game_art.{COLLECTION_EXTENSION}")));
    let trace: Trace = serde_json::from_slice(&std::fs::read(&trace_path)?)?;
    let root = trace_path.parent().unwrap_or_else(|| Path::new("."));
    let mut collection = AssetCollection::new(trace.name);
    collection.elements = trace.elements;
    let mut differing_pixels = 0usize;
    let mut total_pixels = 0usize;
    let mut largest_channel_delta = 0u8;
    let mut compact_nodes = 0usize;
    let mut expanded_nodes = 0usize;
    let nested_element_calls = collection
        .elements
        .iter()
        .flat_map(|element| &element.graph.nodes)
        .filter(|node| matches!(node.node, GeneratorNode::ElementCall { .. }))
        .count();
    let mut asset_element_calls = 0usize;

    for asset in trace.assets {
        println!("evaluating {}", asset.id);
        let image = image::open(root.join(&asset.image))?.into_rgba8();
        if image.dimensions() != (asset.width, asset.height) {
            return Err(format!(
                "{} is {}x{}, expected {}x{}",
                asset.image,
                image.width(),
                image.height(),
                asset.width,
                asset.height
            )
            .into());
        }
        let pixels = WrappedImage::from_parts(
            (asset.width as i32, asset.height as i32).into(),
            image.as_raw(),
        );
        compact_nodes += asset.graph.nodes.len();
        asset_element_calls += asset
            .graph
            .nodes
            .iter()
            .filter(|node| matches!(node.node, GeneratorNode::ElementCall { .. }))
            .count();
        let expanded = spriterize::gui::graph::expand_elements(&asset.graph, &collection.elements)
            .map_err(|error| format!("{} elements are invalid: {error}", asset.id))?;
        expanded_nodes += expanded.nodes.len();
        let graph = spriterize::gui::graph::from_recipe(&expanded)
            .map_err(|error| format!("{} graph is invalid: {error}", asset.id))?;
        let generated =
            spriterize::gui::graph::evaluate(&graph, asset.width as usize, asset.height as usize)
                .map_err(|error| format!("{} graph failed: {error}", asset.id))?;
        let mut asset_differing_pixels = 0usize;
        let mut asset_largest_delta = 0u8;
        let mut asset_alpha_differences = 0usize;
        let mut asset_largest_alpha_delta = 0u8;
        for (reference, traced) in image
            .as_raw()
            .chunks_exact(4)
            .zip(generated.chunks_exact(4))
        {
            total_pixels += 1;
            if reference != traced {
                differing_pixels += 1;
                asset_differing_pixels += 1;
            }
            for (a, b) in reference.iter().zip(traced) {
                let delta = a.abs_diff(*b);
                largest_channel_delta = largest_channel_delta.max(delta);
                asset_largest_delta = asset_largest_delta.max(delta);
            }
            let alpha_delta = reference[3].abs_diff(traced[3]);
            asset_alpha_differences += usize::from(alpha_delta != 0);
            asset_largest_alpha_delta = asset_largest_alpha_delta.max(alpha_delta);
        }
        if asset_differing_pixels != 0 {
            let has_noise = asset.graph.nodes.iter().any(|node| {
                matches!(
                    node.node,
                    GeneratorNode::Perlin { .. }
                        | GeneratorNode::ValueNoise { .. }
                        | GeneratorNode::Worley { .. }
                        | GeneratorNode::Fbm { .. }
                        | GeneratorNode::Ridged { .. }
                )
            });
            let asset_pixels = asset.width as usize * asset.height as usize;
            println!(
                "  fidelity: {asset_differing_pixels}/{asset_pixels} ({:.2}%), max delta {asset_largest_delta}; alpha {asset_alpha_differences}, max {asset_largest_alpha_delta}; noise={has_noise}",
                100.0 * asset_differing_pixels as f64 / asset_pixels as f64
            );
        }
        let mut state = State::<WrappedImage>::new(
            (asset.width as i32, asset.height as i32).into(),
            None,
            None,
        );
        state.set_layer_generator(0, Some(Generator::graph(asset.graph)), Some(pixels))?;
        collection.projects.push(CollectionProject {
            id: asset.id.clone(),
            name: asset.name,
            entry: format!("projects/{}.spriterize", asset.id),
            output: asset.output,
            export: Default::default(),
            metadata: AssetMetadata {
                slice9: asset.slice9,
                tags: asset.tags,
            },
            data: state.project_bytes()?,
        });
    }

    collection.format_version = FORMAT_VERSION;
    collection.save(&output)?;
    let loaded = AssetCollection::load(&output)?;
    if loaded.projects.len() != collection.projects.len() {
        return Err("saved collection did not load back with all projects".into());
    }
    println!(
        "wrote {} evaluated, editable graph projects to {}",
        collection.projects.len(),
        output.display()
    );
    println!(
        "Rust rerun differs from the Python reference at {differing_pixels}/{total_pixels} pixels; largest channel delta {largest_channel_delta}"
    );
    println!(
        "{} shared elements keep stored asset graphs at {compact_nodes} nodes instead of {expanded_nodes} expanded nodes",
        collection.elements.len()
    );
    println!(
        "stored graphs contain {asset_element_calls} asset element calls and {nested_element_calls} nested element calls"
    );
    Ok(())
}
