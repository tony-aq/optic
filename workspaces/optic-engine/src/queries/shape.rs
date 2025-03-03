use crate::commands::{shape as shape_commands, ShapeCommand};
use crate::projections::shape::{CoreShapeNode, Edge, Node};
use crate::projections::shape::{FieldNode, FieldNodeDescriptor, ShapeNode, ShapeProjection};
use crate::shapes::traverser::{ShapeTrail, ShapeTrailPathComponent};
use crate::state::shape::{FieldId, ShapeId, ShapeKind, ShapeParameterId};
use crate::state::SpecIdGenerator;
use petgraph::visit::EdgeRef;
use std::collections::BTreeSet;
use std::iter::FromIterator;

pub struct ShapeQueries<'a> {
  pub shape_projection: &'a ShapeProjection,
}

#[derive(Clone, Debug)]
pub struct ResolvedTrail<'a> {
  pub core_shape_kind: &'a ShapeKind,
  pub shape_id: ShapeId,
}

impl<'a> ShapeQueries<'a> {
  pub fn new(shape_projection: &'a ShapeProjection) -> Self {
    ShapeQueries { shape_projection }
  }

  pub fn list_trail_choices(&self, shape_trail: &ShapeTrail) -> Vec<ChoiceOutput> {
    let projection = &self.shape_projection;
    let root_node_index = projection.get_shape_node_index(&shape_trail.root_shape_id);

    let mut trail_components = shape_trail.path.iter();

    let mut parent_node_index = root_node_index;
    let mut resolved = ResolvedTrail {
      shape_id: shape_trail.root_shape_id.clone(),
      core_shape_kind: self.resolve_to_core_shape(&shape_trail.root_shape_id),
    };

    // eprintln!("{:?}", resolved);
    while let Some(trail_component) = trail_components.next() {
      // eprintln!("{:?}", trail_component);
      resolved = self.resolve_trail_to_core_shape_helper(&resolved, trail_component);

      parent_node_index = projection.get_shape_node_index(&resolved.shape_id);
      // eprintln!("{:?} -> {:?}", resolved, parent_node_index);
    }

    if let None = parent_node_index {
      return vec![];
    }

    let current_node_index = parent_node_index.unwrap();
    let core_shape_nodes = projection.get_core_shape_nodes(&current_node_index);
    if let None = core_shape_nodes {
      return vec![];
    }

    let result: Vec<ChoiceOutput> = core_shape_nodes
      .unwrap()
      .map(|core_shape_node| {
        let shape_id = match self.shape_projection.graph.node_weight(*current_node_index) {
          Some(Node::Shape(shape_node)) => shape_node.shape_id.clone(),
          _ => unreachable!("expected to be a core shape node"),
        };
        let trails: Vec<ChoiceOutput> = match core_shape_node.descriptor.kind {
          ShapeKind::NullableKind => {
            let nullable_parameter_id = core_shape_node
              .descriptor
              .kind
              .get_parameter_descriptor()
              .unwrap()
              .shape_parameter_id;
            let item_shape_id =
              self.resolve_parameter_to_shape(&shape_id, &String::from(nullable_parameter_id));
            let trail = shape_trail
              .with_component(ShapeTrailPathComponent::NullableTrail {
                shape_id: shape_id.clone(),
              })
              .with_component(ShapeTrailPathComponent::NullableItemTrail {
                shape_id: shape_id.clone(),
                inner_shape_id: item_shape_id.clone(),
              });

            let mut output = vec![ChoiceOutput {
              parent_trail: shape_trail.clone(),
              additional_components: vec![ShapeTrailPathComponent::NullableTrail {
                shape_id: shape_id.clone(),
              }],
              shape_id: shape_id.clone(),
              core_shape_kind: core_shape_node.descriptor.kind.clone(),
            }];
            output.append(&mut self.list_trail_choices(&trail));
            output
          }
          ShapeKind::OptionalKind => {
            let optional_parameter_id = core_shape_node
              .descriptor
              .kind
              .get_parameter_descriptor()
              .unwrap()
              .shape_parameter_id;
            let item_shape_id =
              self.resolve_parameter_to_shape(&shape_id, &String::from(optional_parameter_id));
            let trail = shape_trail
              .with_component(ShapeTrailPathComponent::OptionalTrail {
                shape_id: shape_id.clone(),
              })
              .with_component(ShapeTrailPathComponent::OptionalItemTrail {
                shape_id: shape_id.clone(),
                inner_shape_id: item_shape_id.clone(),
              });
            let mut output = vec![ChoiceOutput {
              parent_trail: shape_trail.clone(),
              additional_components: vec![ShapeTrailPathComponent::OptionalTrail {
                shape_id: shape_id.clone(),
              }],
              shape_id: shape_id.clone(),
              core_shape_kind: core_shape_node.descriptor.kind.clone(),
            }];
            output.append(&mut self.list_trail_choices(&trail));
            output
          }
          ShapeKind::OneOfKind => self
            .resolve_parameters_to_shapes(&shape_id)
            .into_iter()
            .map(|i| {
              let (item_parameter_id, item_shape_id) = i;
              let trail = shape_trail
                .with_component(ShapeTrailPathComponent::OneOfTrail {
                  shape_id: shape_id.clone(),
                })
                .with_component(ShapeTrailPathComponent::OneOfItemTrail {
                  item_shape_id,
                  one_of_id: shape_id.clone(),
                  parameter_id: item_parameter_id,
                });
              self.list_trail_choices(&trail)
            })
            .flatten()
            .collect(),
          _ => vec![ChoiceOutput {
            parent_trail: shape_trail.clone(),
            additional_components: vec![],
            shape_id: shape_id.clone(),
            core_shape_kind: core_shape_node.descriptor.kind.clone(),
          }],
        };
        trails
      })
      .flatten()
      .collect();
    result
  }

  pub fn list_known_trail_choices(&self, shape_trail: &ShapeTrail) -> Vec<ChoiceOutput> {
    self
      .list_trail_choices(shape_trail)
      .into_iter()
      .filter(|choice| !matches!(choice.core_shape_kind, ShapeKind::UnknownKind))
      .collect()
  }

  pub fn resolve_to_core_shape(&self, shape_id: &ShapeId) -> &ShapeKind {
    //@TODO: use petgraph::visit::EdgeFiltered, etc.
    //@GOTCHA: this does not support multiple levels of ancestors
    let shape_node_index = self
      .shape_projection
      .get_shape_node_index(shape_id)
      .expect("shape node to exist for shape id");
    let core_shape_node_index = self
      .shape_projection
      .get_ancestor_shape_node_index(shape_node_index)
      .unwrap();
    // eprintln!(
    //   "{:?} -( IsDescendantOf )-> {:?}",
    //   shape_node_index, core_shape_node_index
    // );
    match self
      .shape_projection
      .graph
      .node_weight(core_shape_node_index)
    {
      Some(Node::CoreShape(core_shape_node)) => &core_shape_node.descriptor.kind,
      _ => unreachable!("all shapes should resolve to a core shape"),
    }
  }

  fn resolve_trail_to_core_shape_helper(
    &self,
    parent: &ResolvedTrail,
    path_component: &'a ShapeTrailPathComponent,
  ) -> ResolvedTrail {
    // eprintln!(
    //   "resolve_trail_to_core_shape_helper {:?}",
    //   parent.core_shape_kind.get_descriptor().name
    // );
    match parent.core_shape_kind {
      ShapeKind::ListKind => match path_component {
        ShapeTrailPathComponent::ListItemTrail {
          item_shape_id,
          list_shape_id,
        } => ResolvedTrail {
          shape_id: item_shape_id.clone(),
          core_shape_kind: self.resolve_to_core_shape(&item_shape_id),
        },
        _ => unreachable!("should only receive ListItemTrail relative to ListKind"),
      },
      ShapeKind::OneOfKind => match path_component {
        ShapeTrailPathComponent::OneOfTrail { shape_id } => ResolvedTrail {
          shape_id: parent.shape_id.clone(),
          core_shape_kind: &ShapeKind::OneOfKind,
        },
        ShapeTrailPathComponent::OneOfItemTrail {
          one_of_id,
          parameter_id,
          item_shape_id,
        } => ResolvedTrail {
          shape_id: item_shape_id.clone(),
          core_shape_kind: self.resolve_to_core_shape(&item_shape_id),
        },
        _ => unreachable!("should only receive OneOfTrail or OneOfItemTrail relative to OneOfKind"),
      },
      ShapeKind::NullableKind => match path_component {
        ShapeTrailPathComponent::NullableTrail { shape_id } => ResolvedTrail {
          shape_id: parent.shape_id.clone(),
          core_shape_kind: &ShapeKind::NullableKind,
        },
        ShapeTrailPathComponent::NullableItemTrail {
          shape_id,
          inner_shape_id,
        } => ResolvedTrail {
          shape_id: inner_shape_id.clone(),
          core_shape_kind: self.resolve_to_core_shape(&inner_shape_id),
        },
        _ => unreachable!(
          "should only receive NullableTrail or NullableItemTrail relative to NullableKind"
        ),
      },
      ShapeKind::OptionalKind => match path_component {
        ShapeTrailPathComponent::OptionalTrail { shape_id } => ResolvedTrail {
          shape_id: parent.shape_id.clone(),
          core_shape_kind: &ShapeKind::OptionalKind,
        },
        ShapeTrailPathComponent::OptionalItemTrail {
          shape_id,
          inner_shape_id,
        } => ResolvedTrail {
          shape_id: inner_shape_id.clone(),
          core_shape_kind: self.resolve_to_core_shape(&inner_shape_id),
        },
        _ => unreachable!(
          "should only receive OptionalTrail or OptionalItemTrail relative to OptionalKind"
        ),
      },
      ShapeKind::ObjectKind => match path_component {
        ShapeTrailPathComponent::ObjectTrail { shape_id } => ResolvedTrail {
          shape_id: shape_id.clone(),
          core_shape_kind: &ShapeKind::ObjectKind,
        },
        ShapeTrailPathComponent::ObjectFieldTrail {
          field_id,
          field_shape_id,
          parent_object_shape_id,
        } => ResolvedTrail {
          shape_id: field_shape_id.clone(),
          core_shape_kind: self.resolve_to_core_shape(&field_shape_id),
        },
        _ => unreachable!("should only receive ObjectFieldTrail relative to ObjectKind"),
      },
      ShapeKind::NumberKind => ResolvedTrail {
        shape_id: parent.shape_id.clone(),
        core_shape_kind: &ShapeKind::NumberKind,
      },
      ShapeKind::BooleanKind => ResolvedTrail {
        shape_id: parent.shape_id.clone(),
        core_shape_kind: &ShapeKind::BooleanKind,
      },
      ShapeKind::StringKind => ResolvedTrail {
        shape_id: parent.shape_id.clone(),
        core_shape_kind: &ShapeKind::StringKind,
      },
      ShapeKind::UnknownKind => ResolvedTrail {
        shape_id: parent.shape_id.clone(),
        core_shape_kind: &ShapeKind::UnknownKind,
      },
      x => {
        //dbg!(x);
        unimplemented!("need to support more shapekinds")
      }
    }
  }

  pub fn resolve_parameter_to_shape(
    &self,
    shape_id: &ShapeId,
    shape_parameter_id: &ShapeParameterId,
  ) -> ShapeId {
    let projection = &self.shape_projection;

    let shape_node_index = projection
      .get_shape_node_index(shape_id)
      .expect("shape id to resolve parameter for must exist");

    let shape_parameter_node_index = projection
      .get_shape_parameter_node_index(shape_parameter_id)
      .unwrap_or_else(|| {
        panic!(
          "shape parameter id '{}' to resolve parameter for must exist",
          shape_parameter_id
        )
      });

    // @REFACTOR: move this to a method on the projection, we shouldn't have
    // to reach into projection internals
    let mut outgoing_edges = projection
      .graph
      .edges_connecting(*shape_node_index, *shape_parameter_node_index);

    let existing_binding = outgoing_edges
      .next()
      .expect("expected a parameter binding to exist");
    let edge_index = existing_binding.id();
    let edge_weight = projection.graph.edge_weight(edge_index).unwrap();
    match edge_weight {
      Edge::HasBinding(b) => b.shape_id.clone(),
      _ => unreachable!("expected edge to be a HasBinding"),
    }
  }

  pub fn resolve_parameters_to_shapes(
    &self,
    shape_id: &ShapeId,
  ) -> Vec<(ShapeParameterId, ShapeId)> {
    let projection = &self.shape_projection;

    let shape_node_index = projection
      .get_shape_node_index(shape_id)
      .expect("shape id to resolve parameter for must exist");

    projection
      .graph
      .edges_directed(*shape_node_index, petgraph::Direction::Outgoing)
      .filter_map(|edge_reference| {
        let edge_weight = edge_reference.weight();
        match edge_weight {
          Edge::HasBinding(b) => {
            let neighbor = projection
              .graph
              .node_weight(edge_reference.target())
              .unwrap();
            match neighbor {
              Node::ShapeParameter(parameter_node) => {
                Some((parameter_node.parameter_id.clone(), b.shape_id.clone()))
              }
              _ => unreachable!("expected HasBinding edge to point to a ShapeParameter"),
            }
          }
          _ => None,
        }
      })
      .collect()
  }

  pub fn resolve_field_id(&self, shape_id: &ShapeId, field_name: &String) -> Option<FieldId> {
    let projection = &self.shape_projection;

    let shape_node_index = *projection
      .get_shape_node_index(shape_id)
      .expect("shape id to which field belongs should exist");

    projection
      .graph
      .edges_directed(shape_node_index, petgraph::Direction::Incoming)
      .find_map(|edge| match edge.weight() {
        Edge::IsFieldOf => match projection.graph.node_weight(edge.source()) {
          Some(Node::Field(ref field_node)) => {
            // eprintln!("resolve_field_id item {:?}", field_node);
            let FieldNode {
              field_id,
              descriptor,
            } = field_node;
            if descriptor.name == *field_name {
              Some(field_id.clone())
            } else {
              // eprintln!(
              //   "did not match descriptor.name {} to field name {}",
              //   &descriptor.name, field_name
              // );
              None
            }
          }
          _ => {
            // eprintln!("did not match node type as field node");
            None
          }
        },
        _ => {
          // eprintln!(
          //   "did not match edge type as isFieldOf variant {:?}",
          //   edge.weight()
          // );
          None
        }
      })
  }

  pub fn resolve_field_shape_node(&self, field_id: &FieldId) -> Option<ShapeId> {
    let projection = &self.shape_projection;

    let field_node_index = *projection
      .get_field_node_index(field_id)
      .expect("field id to which field belongs should exist");

    projection
      .graph
      .edges_directed(field_node_index, petgraph::Direction::Incoming)
      .find_map(|edge| match edge.weight() {
        Edge::BelongsTo => match projection.graph.node_weight(edge.source()) {
          Some(Node::Shape(ref shape_node)) => Some(shape_node.shape_id.clone()),
          _ => None,
        },
        _ => None,
      })
  }

  pub fn resolve_shape_field_id_and_names(
    &self,
    shape_id: &ShapeId,
  ) -> impl Iterator<Item = (&FieldId, &String)> {
    let projection = &self.shape_projection;

    let object_node_index = &projection
      .get_shape_node_index(shape_id)
      .expect("shape id for which to find field nodes should exist");

    projection
      .get_shape_field_nodes(object_node_index)
      .unwrap()
      .map(|field_node| {
        let FieldNode {
          field_id,
          descriptor,
        } = field_node;
        (field_id, &descriptor.name)
      })
  }

  pub fn resolve_shape_trail(&self, shape_id: &ShapeId) -> Option<ShapeTrail> {
    let mut next_node = self.shape_projection.get_node_by_id(shape_id);

    let mut trail_components = vec![];
    let mut root_shape_id = None;

    while let Some((current_node_index, current_node)) = next_node.take() {
      match current_node {
        Node::Shape(shape_node) => {
          let core_shape_kind = self.resolve_to_core_shape(&shape_node.shape_id);

          match core_shape_kind {
            ShapeKind::ListKind => {
              unimplemented!("resolving of shape trail by list shape")
            }
            ShapeKind::ObjectKind => {
              let shape_id = shape_node.shape_id.clone();
              trail_components.push(ShapeTrailPathComponent::ObjectTrail { shape_id });
            }
            ShapeKind::OptionalKind => {
              unimplemented!("resolving of shape trail by optional shape")
            }
            ShapeKind::OneOfKind => {
              unimplemented!("resolving of shape trail by one of shape")
            }
            ShapeKind::NullableKind => {
              unimplemented!("resolving of shape trail by nullable shape")
            }

            ShapeKind::BooleanKind
            | ShapeKind::StringKind
            | ShapeKind::NumberKind
            | ShapeKind::UnknownKind => {
              root_shape_id = None;
            }

            ShapeKind::AnyKind
            | ShapeKind::MapKind
            | ShapeKind::IdentifierKind
            | ShapeKind::ReferenceKind => {
              unimplemented!("resolving of shape trail by complex typed shapes")
            }
          }

          root_shape_id.replace(shape_node.shape_id.clone());
          next_node = self.shape_projection.get_owner_node(&current_node_index);
        }
        Node::Field(field_node) => {
          let field_id = field_node.field_id.clone();
          let field_shape_id = self
            .resolve_field_shape_node(&field_id)
            .expect("a field should describe a shape");

          let owner_node = self.shape_projection.get_owner_node(&current_node_index);
          let parent_object_shape_id = match &owner_node {
            Some((_, Node::Shape(object_shape_node))) => object_shape_node.shape_id.clone(),
            _ => unreachable!("field nodes should be owned by their objects shape node"),
          };

          trail_components.push(ShapeTrailPathComponent::ObjectFieldTrail {
            field_id,
            field_shape_id,
            parent_object_shape_id,
          });

          next_node = owner_node
        }
        Node::ShapeParameter(parameter_node) => {
          unimplemented!("resolving of shape trail by shape parameter");
          // TODO: consider supporting list items here

          // next_node = self.shape_projection.get_owner_node(&current_node_index);
        }
        Node::BatchCommit(_) | Node::CoreShape(_) => {}
      };
    }

    trail_components.reverse();
    root_shape_id.map(move |root_shape_id| ShapeTrail {
      root_shape_id,
      path: trail_components,
    })
  }

  pub fn remove_field_commands(
    &self,
    field_id: &FieldId,
  ) -> Option<impl Iterator<Item = ShapeCommand>> {
    let (field_node_index, field_node) = self.shape_projection.get_field_node(field_id)?; // make sure the field is known

    let command = ShapeCommand::remove_field(field_node.field_id.clone());
    Some(std::iter::once(command))
  }

  pub fn edit_shape_trail_commands(
    &'a self,
    shape_trail: &ShapeTrail,
    requested_kinds: impl IntoIterator<Item = &'a ShapeKind>,
    id_generator: &mut impl SpecIdGenerator,
  ) -> Option<impl Iterator<Item = ShapeCommand>> {
    let current_trail_choices = self.list_trail_choices(shape_trail);

    let current_kinds: BTreeSet<_> = current_trail_choices
      .iter()
      .map(|choice| &choice.core_shape_kind)
      .cloned()
      .collect();

    let togglable_kinds = if shape_trail.is_field() {
      BTreeSet::from_iter(vec![ShapeKind::OptionalKind, ShapeKind::NullableKind])
    } else {
      BTreeSet::new() // only allow edits to fields for now
    };

    let primitive_choice = current_trail_choices
      .into_iter()
      .filter(|choice| !togglable_kinds.contains(&choice.core_shape_kind))
      .next()?;

    let subject_shape_id = if shape_trail.is_field() {
      primitive_choice.parent_trail.path.iter().rev().find_map(
        |path_component| match path_component {
          ShapeTrailPathComponent::OptionalItemTrail { inner_shape_id, .. } => Some(inner_shape_id),
          ShapeTrailPathComponent::NullableItemTrail { inner_shape_id, .. } => Some(inner_shape_id),
          ShapeTrailPathComponent::ObjectFieldTrail { field_shape_id, .. } => Some(field_shape_id),
          _ => None,
        },
      )
    } else {
      None
    }?; // nothing to update if there isn't a subject

    let required_kinds: BTreeSet<_> = requested_kinds
      .into_iter()
      .filter(|kind| togglable_kinds.contains(kind))
      .cloned()
      .collect();

    let mut new_shape_prototypes = vec![];
    let mut root_shape_id = subject_shape_id.clone();

    if required_kinds.contains(&ShapeKind::NullableKind) {
      let subject_shape_id = root_shape_id.clone(); // wrapping the current shape
      let prototype = ShapePrototype {
        id: id_generator.shape(),
        prototype_descriptor: ShapePrototypeDescriptor::NullableShape { subject_shape_id },
      };

      root_shape_id = prototype.id.clone();
      new_shape_prototypes.push(prototype);
    }

    if required_kinds.contains(&ShapeKind::OptionalKind) {
      let subject_shape_id = root_shape_id.clone(); // wrapping the current shape
      let prototype = ShapePrototype {
        id: id_generator.shape(),
        prototype_descriptor: ShapePrototypeDescriptor::OptionalShape { subject_shape_id },
      };

      root_shape_id = prototype.id.clone();
      new_shape_prototypes.push(prototype);
    }

    let shape_commands = new_shape_prototypes
      .into_iter()
      .flat_map(ShapePrototype::into_commands);

    let additional_commands = if shape_trail.is_field() {
      let field_id = shape_trail
        .last_field_id()
        .expect("there should be a last field id if the trail described a field");

      vec![ShapeCommand::set_field_shape(
        field_id.clone(),
        root_shape_id,
      )]
    } else {
      vec![]
    };

    let commands = shape_commands.chain(additional_commands);
    Some(commands)
  }
}

#[derive(Clone, Debug)]
pub struct ChoiceOutput {
  pub parent_trail: ShapeTrail,
  pub additional_components: Vec<ShapeTrailPathComponent>,
  pub shape_id: ShapeId,
  pub core_shape_kind: ShapeKind,
}

impl ChoiceOutput {
  pub fn shape_trail(&self) -> ShapeTrail {
    let mut path = self.parent_trail.path.clone();
    let mut additional_components = self.additional_components.clone();
    path.append(&mut additional_components);
    ShapeTrail {
      root_shape_id: self.parent_trail.root_shape_id.clone(),
      path,
    }
  }
}

#[derive(Clone, Debug)]
struct ShapePrototype {
  id: ShapeId,
  prototype_descriptor: ShapePrototypeDescriptor,
}

#[derive(Clone, Debug)]
enum ShapePrototypeDescriptor {
  OptionalShape { subject_shape_id: ShapeId },
  NullableShape { subject_shape_id: ShapeId },
  PrimitiveShape { base_shape_kind: ShapeKind },
}

impl ShapePrototype {
  fn into_commands(self) -> Vec<ShapeCommand> {
    match self.prototype_descriptor {
      ShapePrototypeDescriptor::OptionalShape { subject_shape_id } => {
        let parameter_id = ShapeKind::OptionalKind
          .get_parameter_descriptor()
          .unwrap()
          .shape_parameter_id;

        vec![
          ShapeCommand::add_shape(self.id.clone(), ShapeKind::OptionalKind, String::from("")),
          ShapeCommand::set_parameter_shape(
            self.id.clone(),
            parameter_id.to_owned(),
            subject_shape_id,
          ),
        ]
      }
      ShapePrototypeDescriptor::NullableShape { subject_shape_id } => {
        let parameter_id = ShapeKind::NullableKind
          .get_parameter_descriptor()
          .unwrap()
          .shape_parameter_id;

        vec![
          ShapeCommand::add_shape(self.id.clone(), ShapeKind::NullableKind, String::from("")),
          ShapeCommand::set_parameter_shape(
            self.id.clone(),
            parameter_id.to_owned(),
            subject_shape_id,
          ),
        ]
      }
      ShapePrototypeDescriptor::PrimitiveShape { base_shape_kind } => unimplemented!(),
    }
  }
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::commands::SpecCommand;
  use crate::events::SpecEvent;
  use crate::projections::SpecProjection;
  use crate::Aggregate;
  use insta::assert_debug_snapshot;
  use serde_json::json;

  #[test]
  pub fn can_generate_remove_field_commands() {
    let events: Vec<SpecEvent> = serde_json::from_value(json!([
      { "ShapeAdded": { "shapeId": "string_shape_1", "baseShapeId": "$string", "name": "", "eventContext": null }},
      { "ShapeAdded": { "shapeId": "object_shape_1", "baseShapeId": "$object", "name": "", "eventContext": null }},
      { "FieldAdded": { "fieldId": "field_1", "shapeId": "object_shape_1", "name": "lastName", "shapeDescriptor": { "FieldShapeFromShape": { "fieldId": "field_1", "shapeId": "string_shape_1"}}, "eventContext": null }},
    ]))
    .expect("should be able to deserialize test events");

    let spec_projection = SpecProjection::from(events);

    let shape_queries = ShapeQueries::new(spec_projection.shape());

    let remove_field_commands = shape_queries
      .remove_field_commands(&String::from("field_1"))
      .expect("commands to remove fields should generate for existing field")
      .map(SpecCommand::from)
      .collect::<Vec<_>>();

    assert_debug_snapshot!(
      "can_generate_remove_field_commands__commands",
      &remove_field_commands
    );

    let updated_spec = assert_valid_commands(spec_projection.clone(), remove_field_commands);
    let choice_mapping = updated_spec.shape().to_choice_mapping();

    assert_debug_snapshot!(
      "can_generate_remove_field_commands__choice_mapping",
      choice_mapping
    );
  }

  #[test]
  pub fn can_generate_edit_shape_trail_commands_to_make_field_optional() {
    let events: Vec<SpecEvent> = serde_json::from_value(json!([
      { "ShapeAdded": { "shapeId": "string_shape_1", "baseShapeId": "$string", "name": "", "eventContext": null }},
      { "ShapeAdded": { "shapeId": "object_shape_1", "baseShapeId": "$object", "name": "", "eventContext": null }},
      { "FieldAdded": { "fieldId": "field_1", "shapeId": "object_shape_1", "name": "lastName", "shapeDescriptor": { "FieldShapeFromShape": { "fieldId": "field_1", "shapeId": "string_shape_1"}}, "eventContext": null }},
    ]))
    .expect("should be able to deserialize test events");

    let spec_projection = SpecProjection::from(events);
    let shape_queries = ShapeQueries::new(spec_projection.shape());
    let mut id_generator = SequentialIdGenerator { next_id: 1093 }; // <3 primes

    let shape_trail = ShapeTrail::new(String::from("object_shape_1")).with_component(
      ShapeTrailPathComponent::ObjectFieldTrail {
        field_id: String::from("field_1"),
        field_shape_id: String::from("string_shape_1"),
        parent_object_shape_id: String::from("object_shape_1"),
      },
    );
    let required_kinds = vec![ShapeKind::OptionalKind];
    let edit_shape_commands = shape_queries
      .edit_shape_trail_commands(&shape_trail, &required_kinds, &mut id_generator)
      .expect("field should be able to be made optional")
      .map(SpecCommand::from)
      .collect::<Vec<_>>();

    assert_debug_snapshot!(
      "can_generate_edit_shape_trail_commands_to_make_field_optional__commands",
      &edit_shape_commands
    );

    let updated_spec = assert_valid_commands(spec_projection.clone(), edit_shape_commands);
    let choice_mapping = updated_spec.shape().to_choice_mapping();

    assert_debug_snapshot!(
      "can_generate_edit_shape_trail_commands_to_make_field_optional__choice_mapping",
      choice_mapping
    );
  }

  #[test]
  pub fn can_generate_edit_shape_trail_commands_to_make_field_nullable() {
    let events: Vec<SpecEvent> = serde_json::from_value(json!([
      { "ShapeAdded": { "shapeId": "string_shape_1", "baseShapeId": "$string", "name": "" }},
      { "ShapeAdded": { "shapeId": "object_shape_1", "baseShapeId": "$object", "name": "" }},
      { "FieldAdded": { "fieldId": "field_1", "shapeId": "object_shape_1", "name": "lastName", "shapeDescriptor": { "FieldShapeFromShape": { "fieldId": "field_1", "shapeId": "string_shape_1"}} }},
    ]))
    .expect("should be able to deserialize test events");

    let spec_projection = SpecProjection::from(events);
    let shape_queries = ShapeQueries::new(spec_projection.shape());
    let mut id_generator = SequentialIdGenerator { next_id: 1093 }; // <3 primes

    let shape_trail = ShapeTrail::new(String::from("object_shape_1")).with_component(
      ShapeTrailPathComponent::ObjectFieldTrail {
        field_id: String::from("field_1"),
        field_shape_id: String::from("string_shape_1"),
        parent_object_shape_id: String::from("object_shape_1"),
      },
    );
    let required_kinds = vec![ShapeKind::NullableKind];
    let edit_shape_commands = shape_queries
      .edit_shape_trail_commands(&shape_trail, &required_kinds, &mut id_generator)
      .expect("field should be able to be made nullable")
      .map(SpecCommand::from)
      .collect::<Vec<_>>();

    assert_debug_snapshot!(
      "can_generate_edit_shape_trail_commands_to_make_field_nullable__commands",
      &edit_shape_commands
    );

    let updated_spec = assert_valid_commands(spec_projection.clone(), edit_shape_commands);
    let choice_mapping = updated_spec.shape().to_choice_mapping();

    assert_debug_snapshot!(
      "can_generate_edit_shape_trail_commands_to_make_field_nullable__choice_mapping",
      choice_mapping
    );
  }

  #[test]
  pub fn can_generate_edit_shape_trail_commands_to_make_unknown_nullable_field_optional() {
    let events: Vec<SpecEvent> = serde_json::from_value(json!([
      { "ShapeAdded": { "shapeId": "object_shape_1", "baseShapeId": "$object", "name": "" }},
      { "ShapeAdded": { "shapeId": "unknown_shape_1", "baseShapeId": "$unknown", "name": "" }},
      { "ShapeAdded": { "shapeId": "nullable_shape_1", "baseShapeId": "$nullable", "name": "" }},
      { "ShapeParameterShapeSet": { "shapeDescriptor": { "ProviderInShape": { "shapeId": "nullable_shape_1","providerDescriptor": {"ShapeProvider": {"shapeId": "unknown_shape_1"}},"consumingParameterId": "$nullableInner" }}}},
      { "FieldAdded": { "fieldId": "field_1", "shapeId": "object_shape_1", "name": "lastName", "shapeDescriptor": { "FieldShapeFromShape": { "fieldId": "field_1", "shapeId": "nullable_shape_1"}} }},
    ]))
    .expect("should be able to deserialize test events");

    let spec_projection = SpecProjection::from(events);
    let shape_queries = ShapeQueries::new(spec_projection.shape());
    let mut id_generator = SequentialIdGenerator { next_id: 1093 }; // <3 primes

    let shape_trail = ShapeTrail::new(String::from("object_shape_1"))
      .with_component(ShapeTrailPathComponent::ObjectTrail {
        shape_id: String::from("object_shape_1"),
      })
      .with_component(ShapeTrailPathComponent::ObjectFieldTrail {
        field_id: String::from("field_1"),
        field_shape_id: String::from("nullable_shape_1"),
        parent_object_shape_id: String::from("object_shape_1"),
      });
    let required_kinds = vec![ShapeKind::OptionalKind, ShapeKind::NullableKind];
    let edit_shape_commands = shape_queries
      .edit_shape_trail_commands(&shape_trail, &required_kinds, &mut id_generator)
      .expect("field should be able to be made optional")
      .map(SpecCommand::from)
      .collect::<Vec<_>>();

    assert_debug_snapshot!(
      "can_generate_edit_shape_trail_commands_to_make_unknown_nullable_field_optional__commands",
      &edit_shape_commands
    );

    let updated_spec = assert_valid_commands(spec_projection.clone(), edit_shape_commands);
    let choice_mapping = updated_spec.shape().to_choice_mapping();

    assert_debug_snapshot!(
      "can_generate_edit_shape_trail_commands_to_make_unknown_nullable_field_optional__choice_mapping",
      choice_mapping
    );
  }

  #[test]
  pub fn can_generate_edit_shape_trail_commands_to_make_optional_field_nullable() {
    let events: Vec<SpecEvent> = serde_json::from_value(json!([
      { "ShapeAdded": { "shapeId": "string_shape_1", "baseShapeId": "$string", "name": "" }},
      { "ShapeAdded": { "shapeId": "object_shape_1", "baseShapeId": "$object", "name": "" }},
      { "ShapeAdded": { "shapeId": "optional_shape_1", "baseShapeId": "$optional", "name": "" }},
      { "ShapeParameterShapeSet": { "shapeDescriptor": { "ProviderInShape": { "shapeId": "optional_shape_1","providerDescriptor": {"ShapeProvider": {"shapeId": "string_shape_1"}},"consumingParameterId": "$optionalInner" }}}},
      { "FieldAdded": { "fieldId": "field_1", "shapeId": "object_shape_1", "name": "lastName", "shapeDescriptor": { "FieldShapeFromShape": { "fieldId": "field_1", "shapeId": "optional_shape_1"}} }},
    ]))
    .expect("should be able to deserialize test events");

    let spec_projection = SpecProjection::from(events);
    let shape_queries = ShapeQueries::new(spec_projection.shape());
    let mut id_generator = SequentialIdGenerator { next_id: 1093 }; // <3 primes

    let shape_trail = ShapeTrail::new(String::from("object_shape_1")).with_component(
      ShapeTrailPathComponent::ObjectFieldTrail {
        field_id: String::from("field_1"),
        field_shape_id: String::from("optional_shape_1"),
        parent_object_shape_id: String::from("object_shape_1"),
      },
    );
    let required_kinds = vec![ShapeKind::StringKind, ShapeKind::NullableKind]; // string kind should be filtered out
    let edit_shape_commands = shape_queries
      .edit_shape_trail_commands(&shape_trail, &required_kinds, &mut id_generator)
      .expect("field should be able to be made nullable and optional")
      .map(SpecCommand::from)
      .collect::<Vec<_>>();

    assert_debug_snapshot!(
      "can_generate_edit_shape_trail_commands_to_make_optional_field_nullable__commands",
      &edit_shape_commands
    );

    let updated_spec = assert_valid_commands(spec_projection.clone(), edit_shape_commands);
    let choice_mapping = updated_spec.shape().to_choice_mapping();

    assert_debug_snapshot!(
      "can_generate_edit_shape_trail_commands_to_make_optional_field_nullable__choice_mapping",
      choice_mapping
    );
  }

  #[test]
  pub fn can_generate_edit_shape_trail_commands_to_make_optional_field_nullable_and_optional() {
    let events: Vec<SpecEvent> = serde_json::from_value(json!([
      { "ShapeAdded": { "shapeId": "string_shape_1", "baseShapeId": "$string", "name": "" }},
      { "ShapeAdded": { "shapeId": "object_shape_1", "baseShapeId": "$object", "name": "" }},
      { "ShapeAdded": { "shapeId": "optional_shape_1", "baseShapeId": "$optional", "name": "" }},
      { "ShapeParameterShapeSet": { "shapeDescriptor": { "ProviderInShape": { "shapeId": "optional_shape_1","providerDescriptor": {"ShapeProvider": {"shapeId": "string_shape_1"}},"consumingParameterId": "$optionalInner" }}}},
      { "FieldAdded": { "fieldId": "field_1", "shapeId": "object_shape_1", "name": "lastName", "shapeDescriptor": { "FieldShapeFromShape": { "fieldId": "field_1", "shapeId": "optional_shape_1"}} }},
    ]))
    .expect("should be able to deserialize test events");

    let spec_projection = SpecProjection::from(events);
    let shape_queries = ShapeQueries::new(spec_projection.shape());
    let mut id_generator = SequentialIdGenerator { next_id: 1093 }; // <3 primes

    let shape_trail = ShapeTrail::new(String::from("object_shape_1")).with_component(
      ShapeTrailPathComponent::ObjectFieldTrail {
        field_id: String::from("field_1"),
        field_shape_id: String::from("optional_shape_1"),
        parent_object_shape_id: String::from("object_shape_1"),
      },
    );
    let required_kinds = vec![ShapeKind::NullableKind, ShapeKind::OptionalKind];
    let edit_shape_commands = shape_queries
      .edit_shape_trail_commands(&shape_trail, &required_kinds, &mut id_generator)
      .expect("field should be able to be made nullable and optional")
      .map(SpecCommand::from)
      .collect::<Vec<_>>();

    assert_debug_snapshot!(
      "can_generate_edit_shape_trail_commands_to_make_optional_field_nullable_and_optional__commands",
      &edit_shape_commands
    );

    let updated_spec = assert_valid_commands(spec_projection.clone(), edit_shape_commands);
    let choice_mapping = updated_spec.shape().to_choice_mapping();

    assert_debug_snapshot!(
      "can_generate_edit_shape_trail_commands_to_make_optional_field_nullable_and_optional__choice_mapping",
      choice_mapping
    );
  }

  #[test]
  pub fn can_generate_edit_shape_trail_commands_to_make_polymorphic_field_optional() {
    let events: Vec<SpecEvent> = serde_json::from_value(json!([
      { "ShapeAdded": { "shapeId": "string_shape_1", "baseShapeId": "$string", "name": "" }},
      { "ShapeAdded": { "shapeId": "number_shape_1", "baseShapeId": "$number", "name": "" }},
      { "ShapeAdded": { "shapeId": "object_shape_1", "baseShapeId": "$object", "name": "" }},
      { "ShapeAdded": { "shapeId": "one_of_shape_1", "baseShapeId": "$oneOf", "name": "" }},
      { "ShapeParameterAdded": { "shapeId": "one_of_shape_1", "shapeParameterId": "one_of_param_1", "name": "", "shapeDescriptor": { "ProviderInShape": {"shapeId": "one_of_shape_1","providerDescriptor": { "NoProvider": {} },"consumingParameterId": "one_of_param_1"}}}},
      { "ShapeParameterAdded": { "shapeId": "one_of_shape_1", "shapeParameterId": "one_of_param_2", "name": "", "shapeDescriptor": { "ProviderInShape": {"shapeId": "one_of_shape_1","providerDescriptor": { "NoProvider": {} },"consumingParameterId": "one_of_param_2"}}}},
      { "ShapeParameterShapeSet": { "shapeDescriptor": { "ProviderInShape": { "shapeId": "one_of_shape_1","providerDescriptor": {"ShapeProvider": {"shapeId": "string_shape_1"}},"consumingParameterId": "one_of_param_1" }}}},
      { "ShapeParameterShapeSet": { "shapeDescriptor": { "ProviderInShape": { "shapeId": "one_of_shape_1","providerDescriptor": {"ShapeProvider": {"shapeId": "number_shape_1"}},"consumingParameterId": "one_of_param_2" }}}},
      { "FieldAdded": { "fieldId": "field_1", "shapeId": "object_shape_1", "name": "lastName", "shapeDescriptor": { "FieldShapeFromShape": { "fieldId": "field_1", "shapeId": "one_of_shape_1"}} }},
    ]))
    .expect("should be able to deserialize test events");

    let spec_projection = SpecProjection::from(events);
    let shape_queries = ShapeQueries::new(spec_projection.shape());
    let mut id_generator = SequentialIdGenerator { next_id: 1093 }; // <3 primes

    let shape_trail = ShapeTrail::new(String::from("object_shape_1")).with_component(
      ShapeTrailPathComponent::ObjectFieldTrail {
        field_id: String::from("field_1"),
        field_shape_id: String::from("one_of_shape_1"),
        parent_object_shape_id: String::from("object_shape_1"),
      },
    );
    let required_kinds = vec![ShapeKind::OptionalKind];
    let edit_shape_commands = shape_queries
      .edit_shape_trail_commands(&shape_trail, &required_kinds, &mut id_generator)
      .expect("field should be able to be made nullable and optional")
      .map(SpecCommand::from)
      .collect::<Vec<_>>();

    assert_debug_snapshot!(
      "can_generate_edit_shape_trail_commands_to_make_polymorphic_field_optional__commands",
      &edit_shape_commands
    );

    let updated_spec = assert_valid_commands(spec_projection.clone(), edit_shape_commands);
    let choice_mapping = updated_spec.shape().to_choice_mapping();

    assert_debug_snapshot!(
      "can_generate_edit_shape_trail_commands_to_make_polymorphic_field_optional__choice_mapping",
      choice_mapping
    );
  }

  #[test]
  pub fn can_generate_edit_shape_trail_commands_to_make_optional_nullable_field_required() {
    let events: Vec<SpecEvent> = serde_json::from_value(json!([
      { "ShapeAdded": { "shapeId": "string_shape_1", "baseShapeId": "$string", "name": "" }},
      { "ShapeAdded": { "shapeId": "object_shape_1", "baseShapeId": "$object", "name": "" }},
      { "ShapeAdded": { "shapeId": "nullable_shape_1", "baseShapeId": "$nullable", "name": "" }},
      { "ShapeParameterShapeSet": { "shapeDescriptor": { "ProviderInShape": { "shapeId": "nullable_shape_1","providerDescriptor": {"ShapeProvider": {"shapeId": "string_shape_1"}},"consumingParameterId": "$nullableInner" }}}},
      { "ShapeAdded": { "shapeId": "optional_shape_1", "baseShapeId": "$optional", "name": "" }},
      { "ShapeParameterShapeSet": { "shapeDescriptor": { "ProviderInShape": { "shapeId": "optional_shape_1","providerDescriptor": {"ShapeProvider": {"shapeId": "nullable_shape_1"}},"consumingParameterId": "$optionalInner" }}}},
      { "FieldAdded": { "fieldId": "field_1", "shapeId": "object_shape_1", "name": "lastName", "shapeDescriptor": { "FieldShapeFromShape": { "fieldId": "field_1", "shapeId": "optional_shape_1"}} }},
    ]))
    .expect("should be able to deserialize test events");

    let spec_projection = SpecProjection::from(events);
    let shape_queries = ShapeQueries::new(spec_projection.shape());
    let mut id_generator = SequentialIdGenerator { next_id: 1093 }; // <3 primes

    let shape_trail = ShapeTrail::new(String::from("object_shape_1")).with_component(
      ShapeTrailPathComponent::ObjectFieldTrail {
        field_id: String::from("field_1"),
        field_shape_id: String::from("optional_shape_1"),
        parent_object_shape_id: String::from("object_shape_1"),
      },
    );
    let required_kinds = vec![];
    let edit_shape_commands = shape_queries
      .edit_shape_trail_commands(&shape_trail, &required_kinds, &mut id_generator)
      .expect("field should be able to be made required")
      .map(SpecCommand::from)
      .collect::<Vec<_>>();

    assert_debug_snapshot!(
      "can_generate_edit_shape_trail_commands_to_make_optional_nullable_field_required__commands",
      &edit_shape_commands
    );

    let updated_spec = assert_valid_commands(spec_projection.clone(), edit_shape_commands);
    let choice_mapping = updated_spec.shape().to_choice_mapping();

    assert_debug_snapshot!(
      "can_generate_edit_shape_trail_commands_to_make_optional_nullable_field_required__choice_mapping",
      choice_mapping
    );
  }

  #[test]
  pub fn can_resolve_shape_trails_for_fields() {
    let events: Vec<SpecEvent> = serde_json::from_value(json!([
      { "ShapeAdded": { "shapeId": "object_shape_1", "baseShapeId": "$object", "name": "" }},

      // optional and nullable string field
      { "ShapeAdded": { "shapeId": "string_shape_1", "baseShapeId": "$string", "name": "" }},
      { "ShapeAdded": { "shapeId": "nullable_shape_1", "baseShapeId": "$nullable", "name": "" }},
      { "ShapeParameterShapeSet": { "shapeDescriptor": { "ProviderInShape": { "shapeId": "nullable_shape_1","providerDescriptor": {"ShapeProvider": {"shapeId": "string_shape_1"}},"consumingParameterId": "$nullableInner" }}}},
      { "ShapeAdded": { "shapeId": "optional_shape_1", "baseShapeId": "$optional", "name": "" }},
      { "ShapeParameterShapeSet": { "shapeDescriptor": { "ProviderInShape": { "shapeId": "optional_shape_1","providerDescriptor": {"ShapeProvider": {"shapeId": "nullable_shape_1"}},"consumingParameterId": "$optionalInner" }}}},
      { "FieldAdded": { "fieldId": "field_1", "shapeId": "object_shape_1", "name": "lastName", "shapeDescriptor": { "FieldShapeFromShape": { "fieldId": "field_1", "shapeId": "optional_shape_1"}} }},

      // nested list of strings
      { "ShapeAdded": { "shapeId": "list_shape_1", "baseShapeId": "$list", "name": "" }},
      { "ShapeAdded": { "shapeId": "string_shape_2", "baseShapeId": "$string", "name": "" }},
      { "ShapeParameterShapeSet": { "shapeDescriptor": { "ProviderInShape": { "shapeId": "list_shape_1","providerDescriptor": {"ShapeProvider": {"shapeId": "string_shape_2"}},"consumingParameterId": "$listItem" }}}},
      { "FieldAdded": { "fieldId": "field_2", "shapeId": "object_shape_1", "name": "words", "shapeDescriptor": { "FieldShapeFromShape": { "fieldId": "field_2", "shapeId": "list_shape_1"}} }},

      // nested object
      { "ShapeAdded": { "shapeId": "object_shape_2", "baseShapeId": "$object", "name": "" }},
      { "ShapeAdded": { "shapeId": "boolean_shape_1", "baseShapeId": "$boolean", "name": "" }},
      { "FieldAdded": { "fieldId": "field_3", "shapeId": "object_shape_2", "name": "flag", "shapeDescriptor": { "FieldShapeFromShape": { "fieldId": "field_3", "shapeId": "boolean_shape_1"}} }},
      { "FieldAdded": { "fieldId": "field_4", "shapeId": "object_shape_1", "name": "nestedObject", "shapeDescriptor": { "FieldShapeFromShape": { "fieldId": "field_4", "shapeId": "object_shape_2"}} }},

      // one of number or string
      { "ShapeAdded": { "shapeId": "string_shape_3", "baseShapeId": "$string", "name": "" }},
      { "ShapeAdded": { "shapeId": "number_shape_1", "baseShapeId": "$number", "name": "" }},
      { "ShapeAdded": { "shapeId": "one_of_shape_1", "baseShapeId": "$oneOf", "name": "" }},
      { "ShapeParameterAdded": { "shapeId": "one_of_shape_1", "shapeParameterId": "one_of_param_1", "name": "", "shapeDescriptor": { "ProviderInShape": {"shapeId": "one_of_shape_1","providerDescriptor": { "NoProvider": {} },"consumingParameterId": "one_of_param_1"}}}},
      { "ShapeParameterAdded": { "shapeId": "one_of_shape_1", "shapeParameterId": "one_of_param_2", "name": "", "shapeDescriptor": { "ProviderInShape": {"shapeId": "one_of_shape_1","providerDescriptor": { "NoProvider": {} },"consumingParameterId": "one_of_param_2"}}}},
      { "ShapeParameterShapeSet": { "shapeDescriptor": { "ProviderInShape": { "shapeId": "one_of_shape_1","providerDescriptor": {"ShapeProvider": {"shapeId": "string_shape_3"}},"consumingParameterId": "one_of_param_1" }}}},
      { "ShapeParameterShapeSet": { "shapeDescriptor": { "ProviderInShape": { "shapeId": "one_of_shape_1","providerDescriptor": {"ShapeProvider": {"shapeId": "number_shape_1"}},"consumingParameterId": "one_of_param_2" }}}},
      { "FieldAdded": { "fieldId": "field_5", "shapeId": "object_shape_1", "name": "nestedObject", "shapeDescriptor": { "FieldShapeFromShape": { "fieldId": "field_5", "shapeId": "one_of_shape_1"}} }},

      // unknown nullable field
      { "ShapeAdded": { "shapeId": "unknown_shape_1", "baseShapeId": "$string", "name": "" }},
      { "ShapeAdded": { "shapeId": "nullable_shape_2", "baseShapeId": "$nullable", "name": "" }},
      { "ShapeParameterShapeSet": { "shapeDescriptor": { "ProviderInShape": { "shapeId": "nullable_shape_2","providerDescriptor": {"ShapeProvider": {"shapeId": "unknown_shape_1"}},"consumingParameterId": "$nullableInner" }}}},
      { "FieldAdded": { "fieldId": "field_6", "shapeId": "object_shape_1", "name": "unknownField", "shapeDescriptor": { "FieldShapeFromShape": { "fieldId": "field_6", "shapeId": "nullable_shape_2"}} }},
    ]))
    .expect("should be able to deserialize test events");

    let spec_projection = SpecProjection::from(events);
    let shape_queries = ShapeQueries::new(spec_projection.shape());
    dbg!(&petgraph::dot::Dot::with_config(
      &spec_projection.shape().graph,
      &[]
    ));

    let field_trail = shape_queries.resolve_shape_trail(&"field_1".to_owned());
    assert_debug_snapshot!(
      "can_resolve_shape_trails_for_fields__field_trail",
      &field_trail
    );

    let nested_field_trail = shape_queries.resolve_shape_trail(&"field_3".to_owned());
    assert_debug_snapshot!(
      "can_resolve_shape_trails__nested_field_trail",
      &nested_field_trail
    );

    // UNIMPLEMENTED
    // let optional_shape_trail = shape_queries.resolve_shape_trail(&"optional_shape_1".to_owned());
    // assert_debug_snapshot!(
    //   "can_resolve_shape_trails__optional_shape_trail",
    //   &optional_shape_trail
    // );

    // UNIMPLEMENTED
    // let array_item_trail = shape_queries.resolve_shape_trail(&"string_shape_2".to_owned());
    // assert_debug_snapshot!(
    //   "can_resolve_shape_trails__array_item_trail",
    //   &array_item_trail
    // );

    // UNIMPLEMENTED
    // let one_of_trail = shape_queries.resolve_shape_trail(&"one_of_shape_1".to_owned());
    // assert_debug_snapshot!(
    //   "can_resolve_shape_trails__one_of_trail",
    //   &one_of_trail
    // );

    let unknown_nullable_trail = shape_queries.resolve_shape_trail(&"field_6".to_owned());
    assert_debug_snapshot!(
      "can_resolve_shape_trails__unknown_nullable_trail",
      &unknown_nullable_trail
    );
  }

  fn assert_valid_commands(
    mut spec_projection: SpecProjection,
    commands: impl IntoIterator<Item = SpecCommand>,
  ) -> SpecProjection {
    // let mut spec_projection = SpecProjection::default();
    for command in commands {
      let events = spec_projection
        .execute(command)
        .expect("generated commands must be valid");

      for event in events {
        spec_projection.apply(event)
      }
    }

    spec_projection
  }

  #[derive(Debug, Default)]
  struct SequentialIdGenerator {
    next_id: u32,
  }
  impl SpecIdGenerator for SequentialIdGenerator {
    fn generate_id(&mut self, prefix: &str) -> String {
      self.next_id += 1;
      format!("{}{}", prefix, self.next_id.to_string())
    }
  }
}
