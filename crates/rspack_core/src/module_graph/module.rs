use rustc_hash::FxHashSet as HashSet;

use crate::ExportsInfo;
use crate::{
  module_graph::ConnectionId, ChunkGraph, DependencyId, ModuleIdentifier, ModuleIssuer,
  ModuleProfile,
};

#[derive(Debug, Clone)]
pub struct ModuleGraphModule {
  // edges from module to module
  outgoing_connections: HashSet<ConnectionId>,
  incoming_connections: HashSet<ConnectionId>,

  issuer: ModuleIssuer,

  // pub exec_order: usize,
  pub module_identifier: ModuleIdentifier,
  // an quick way to get a module's all dependencies (including its blocks' dependencies)
  // and it is ordered by dependency creation order
  pub(crate) all_dependencies: Vec<DependencyId>,
  pub(crate) pre_order_index: Option<u32>,
  pub post_order_index: Option<u32>,
  pub exports: ExportsInfo,
  pub profile: Option<Box<ModuleProfile>>,
  pub is_async: bool,
  pub depth: Option<usize>,
  pub optimization_bailout: Vec<String>,
}

impl ModuleGraphModule {
  pub fn new(module_identifier: ModuleIdentifier, exports_info: ExportsInfo) -> Self {
    Self {
      outgoing_connections: Default::default(),
      incoming_connections: Default::default(),
      issuer: ModuleIssuer::Unset,
      // exec_order: usize::MAX,
      module_identifier,
      all_dependencies: Default::default(),
      pre_order_index: None,
      post_order_index: None,
      exports: exports_info,
      profile: None,
      is_async: false,
      depth: None,
      optimization_bailout: vec![],
    }
  }

  pub fn id<'chunk_graph>(&self, chunk_graph: &'chunk_graph ChunkGraph) -> &'chunk_graph str {
    let c = chunk_graph.get_module_id(self.module_identifier);
    c.unwrap_or_else(|| panic!("{} module id not found", self.module_identifier))
  }

  pub fn add_incoming_connection(&mut self, connection_id: ConnectionId) {
    self.incoming_connections.insert(connection_id);
  }

  pub fn remove_incoming_connection(&mut self, connection_id: &ConnectionId) {
    self.incoming_connections.remove(connection_id);
  }

  pub fn add_outgoing_connection(&mut self, connection_id: ConnectionId) {
    self.outgoing_connections.insert(connection_id);
  }

  pub fn remove_outgoing_connection(&mut self, connection_id: &ConnectionId) {
    self.outgoing_connections.remove(connection_id);
  }

  pub fn incoming_connections(&self) -> &HashSet<ConnectionId> {
    &self.incoming_connections
  }

  pub fn outgoing_connections(&self) -> &HashSet<ConnectionId> {
    &self.outgoing_connections
  }

  pub fn get_incoming_connections_unordered(&self) -> &HashSet<ConnectionId> {
    &self.incoming_connections
  }

  pub fn get_outgoing_connections_unordered(&self) -> &HashSet<ConnectionId> {
    &self.outgoing_connections
  }

  pub fn set_profile(&mut self, profile: Box<ModuleProfile>) {
    self.profile = Some(profile);
  }

  pub fn get_profile(&self) -> Option<&ModuleProfile> {
    self.profile.as_deref()
  }

  pub fn set_issuer_if_unset(&mut self, issuer: Option<ModuleIdentifier>) {
    if matches!(self.issuer, ModuleIssuer::Unset) {
      self.issuer = ModuleIssuer::from_identifier(issuer);
    }
  }

  pub fn set_issuer(&mut self, issuer: ModuleIssuer) {
    self.issuer = issuer;
  }

  pub fn get_issuer(&self) -> &ModuleIssuer {
    &self.issuer
  }

  pub(crate) fn optimization_bailout_mut(&mut self) -> &mut Vec<String> {
    &mut self.optimization_bailout
  }
}
