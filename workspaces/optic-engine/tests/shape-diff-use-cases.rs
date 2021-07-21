use insta::assert_debug_snapshot;
use petgraph::dot::Dot;
use serde::Deserialize;
use std::collections::HashSet;
use std::path::Path;
use tokio::fs::read_to_string;

use optic_engine::{
  analyze_documented_bodies, diff_interaction, Aggregate, DiffInteractionConfig, HttpInteraction,
  LearnedShapeDiffAffordancesProjection, SpecEvent, SpecProjection, TaggedInput,
};

#[tokio::main]
#[test]
async fn a_known_field_is_missing() {
  let mut capture = DebugCapture::from_name("a known field is missing.json").await;

  let spec = SpecProjection::from(capture.events);
  let interaction = capture.session.samples.remove(0);
  let interaction_pointers: HashSet<_> =
    std::iter::once(String::from("test-interaction-1")).collect();
  let diff_results = diff_interaction(
    &spec,
    interaction.clone(),
    &DiffInteractionConfig::default(),
  );

  let mut learned_shape_diff_affordances =
    LearnedShapeDiffAffordancesProjection::from(diff_results);

  let analysis = analyze_documented_bodies(&spec, interaction)
    .collect::<Vec<_>>()
    .pop()
    .unwrap();

  let tagged_analysis = TaggedInput(analysis, interaction_pointers);
  learned_shape_diff_affordances.apply(tagged_analysis);

  assert_debug_snapshot!(
    "a_known_field_is_missing__affordances",
    learned_shape_diff_affordances.into_iter()
  );
}

#[tokio::main]
#[test]
async fn deeply_nested_fields_inside_of_arrays() {
  let mut capture = DebugCapture::from_name("deeply nested fields inside of arrays.json").await;

  let spec = SpecProjection::from(capture.events);
  let interaction = capture.session.samples.remove(0);
  let interaction_pointers: HashSet<_> =
    std::iter::once(String::from("test-interaction-1")).collect();
  let diff_results = diff_interaction(
    &spec,
    interaction.clone(),
    &DiffInteractionConfig::default(),
  );

  let mut learned_shape_diff_affordances =
    LearnedShapeDiffAffordancesProjection::from(diff_results);

  let analysis = analyze_documented_bodies(&spec, interaction)
    .collect::<Vec<_>>()
    .pop()
    .unwrap();

  let tagged_analysis = TaggedInput(analysis, interaction_pointers);
  learned_shape_diff_affordances.apply(tagged_analysis);

  // TODO: add snapshot, once we figure out how to make the results order stable in a way
  // that fits our performance budget.
}

#[tokio::main]
#[test]
async fn optional_single_string_or_list_of_strings() {
  let mut capture = DebugCapture::from_name("optional single string or list of strings.json").await;

  let spec = SpecProjection::from(capture.events);
  let interaction = capture.session.samples.remove(0);
  let interaction_pointers: HashSet<_> =
    std::iter::once(String::from("test-interaction-1")).collect();

  assert_debug_snapshot!(
    "optional_single_string_or_list_of_strings__shape_graph",
    Dot::with_config(&spec.shape().graph, &[])
  );
  assert_debug_snapshot!(
    "optional_single_string_or_list_of_strings__shape_choice_mapping",
    &spec.shape().to_choice_mapping()
  );

  let diff_results = diff_interaction(
    &spec,
    interaction.clone(),
    &DiffInteractionConfig::default().with_query_params(true),
  );

  assert_eq!(diff_results.len(), 1);
  assert_debug_snapshot!(
    "optional_single_string_or_list_of_strings__diff_results",
    &diff_results
  );

  let mut learned_shape_diff_affordances =
    LearnedShapeDiffAffordancesProjection::from(diff_results);

  let analysis = analyze_documented_bodies(&spec, interaction)
    .collect::<Vec<_>>()
    .pop()
    .unwrap();

  let tagged_analysis = TaggedInput(analysis, interaction_pointers);
  learned_shape_diff_affordances.apply(tagged_analysis);

  // TODO: add snapshot, once we figure out how to make the results order stable in a way
  // that fits our performance budget.
}

#[derive(Deserialize, Debug)]
struct DebugCapture {
  events: Vec<SpecEvent>,
  session: DebugCaptureSession,
}

impl DebugCapture {
  async fn from_name(capture_name: &str) -> Self {
    let capture_path = std::env::current_dir()
      .unwrap()
      .join("tests/fixtures/shape-diff-use-cases")
      .join(capture_name);

    Self::from_file(capture_path).await
  }

  async fn from_file(capture_path: impl AsRef<Path>) -> Self {
    let capture_json = read_to_string(capture_path)
      .await
      .expect("should be able to read the debug capture");

    serde_json::from_str(&capture_json).expect("should be able to deserialize the debug capture")
  }
}

#[derive(Deserialize, Debug)]
struct DebugCaptureSession {
  samples: Vec<HttpInteraction>,
}
