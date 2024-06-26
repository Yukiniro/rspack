use std::{
  path::Path,
  sync::{Arc, Mutex},
};

use rspack_error::{Diagnostic, Result, TWithDiagnosticArray};
use rspack_loader_runner::{LoaderContext, ResourceData};
use rustc_hash::FxHashMap as HashMap;
use tracing::instrument;

use crate::{
  AdditionalChunkRuntimeRequirementsArgs, AdditionalModuleRequirementsArgs, ApplyContext,
  AssetEmittedArgs, BeforeResolveArgs, BoxLoader, BoxModule, BoxedParserAndGeneratorBuilder,
  ChunkContentHash, ChunkHashArgs, Compilation, CompilationHooks, CompilerHooks, CompilerOptions,
  Content, ContentHashArgs, DoneArgs, FactorizeArgs, JsChunkHashArgs, LoaderRunnerContext,
  ModuleIdentifier, ModuleType, NormalModule, NormalModuleAfterResolveArgs, NormalModuleCreateData,
  NormalModuleFactoryHooks, OptimizeChunksArgs, Plugin,
  PluginAdditionalChunkRuntimeRequirementsOutput, PluginAdditionalModuleRequirementsOutput,
  PluginBuildEndHookOutput, PluginChunkHashHookOutput, PluginContext, PluginFactorizeHookOutput,
  PluginJsChunkHashHookOutput, PluginNormalModuleFactoryAfterResolveOutput,
  PluginNormalModuleFactoryBeforeResolveOutput, PluginNormalModuleFactoryCreateModuleHookOutput,
  PluginNormalModuleFactoryModuleHookOutput, PluginRenderChunkHookOutput, PluginRenderHookOutput,
  PluginRenderManifestHookOutput, PluginRenderModuleContentOutput, PluginRenderStartupHookOutput,
  PluginRuntimeRequirementsInTreeOutput, ProcessAssetsArgs, RenderArgs, RenderChunkArgs,
  RenderManifestArgs, RenderModuleContentArgs, RenderStartupArgs, Resolver, ResolverFactory,
  RuntimeRequirementsInTreeArgs, Stats,
};

pub struct PluginDriver {
  pub(crate) options: Arc<CompilerOptions>,
  pub plugins: Vec<Box<dyn Plugin>>,
  pub resolver_factory: Arc<ResolverFactory>,
  // pub registered_parser: HashMap<ModuleType, BoxedParser>,
  pub registered_parser_and_generator_builder: HashMap<ModuleType, BoxedParserAndGeneratorBuilder>,
  /// Collecting error generated by plugin phase, e.g., `Syntax Error`
  pub diagnostics: Arc<Mutex<Vec<Diagnostic>>>,
  pub compiler_hooks: CompilerHooks,
  pub compilation_hooks: CompilationHooks,
  pub normal_module_factory_hooks: NormalModuleFactoryHooks,
}

impl std::fmt::Debug for PluginDriver {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("PluginDriver")
      .field("options", &self.options)
      .field("plugins", &self.plugins)
      // field("registered_parser", &self.registered_parser)
      .field("registered_parser_and_generator_builder", &"{..}")
      .field("diagnostics", &self.diagnostics)
      .finish()
  }
}

impl PluginDriver {
  pub fn new(
    mut options: CompilerOptions,
    plugins: Vec<Box<dyn Plugin>>,
    resolver_factory: Arc<ResolverFactory>,
  ) -> (Arc<Self>, Arc<CompilerOptions>) {
    let mut compiler_hooks = Default::default();
    let mut compilation_hooks = Default::default();
    let mut normal_module_factory_hooks = Default::default();
    let mut registered_parser_and_generator_builder = HashMap::default();
    let mut apply_context = ApplyContext {
      registered_parser_and_generator_builder: &mut registered_parser_and_generator_builder,
      compiler_hooks: &mut compiler_hooks,
      compilation_hooks: &mut compilation_hooks,
      normal_module_factory_hooks: &mut normal_module_factory_hooks,
    };
    for plugin in &plugins {
      plugin
        .apply(
          PluginContext::with_context(&mut apply_context),
          &mut options,
        )
        .expect("TODO:");
    }

    let options = Arc::new(options);

    (
      Arc::new(Self {
        options: options.clone(),
        plugins,
        resolver_factory,
        registered_parser_and_generator_builder,
        diagnostics: Arc::new(Mutex::new(vec![])),
        compiler_hooks,
        compilation_hooks,
        normal_module_factory_hooks,
      }),
      options,
    )
  }

  pub fn take_diagnostic(&self) -> Vec<Diagnostic> {
    let mut diagnostic = self.diagnostics.lock().expect("TODO:");
    std::mem::take(&mut diagnostic)
  }

  /// Read resource with the given `resource_data`
  ///
  /// Warning:
  /// Webpack does not expose this as the documented API, even though you can reach this with `NormalModule.getCompilationHooks(compilation)`.
  /// For the most of time, you would not need this.
  pub async fn read_resource(&self, resource_data: &ResourceData) -> Result<Option<Content>> {
    for plugin in &self.plugins {
      let result = plugin.read_resource(resource_data).await?;
      if result.is_some() {
        return Ok(result);
      }
    }

    Ok(None)
  }

  #[instrument(name = "plugin:module_asset", skip_all)]
  pub async fn module_asset(&self, module: ModuleIdentifier, asset_name: String) -> Result<()> {
    for plugin in &self.plugins {
      plugin.module_asset(module, asset_name.clone()).await?;
    }

    Ok(())
  }

  pub async fn content_hash(&self, args: &ContentHashArgs<'_>) -> Result<ChunkContentHash> {
    let mut result = HashMap::default();
    for plugin in &self.plugins {
      if let Some((source_type, hash_digest)) =
        plugin.content_hash(PluginContext::new(), args).await?
      {
        result.insert(source_type, hash_digest);
      }
    }
    Ok(result)
  }

  pub async fn chunk_hash(&self, args: &mut ChunkHashArgs<'_>) -> PluginChunkHashHookOutput {
    for plugin in &self.plugins {
      plugin.chunk_hash(PluginContext::new(), args).await?
    }
    Ok(())
  }

  pub async fn render_manifest(
    &self,
    args: RenderManifestArgs<'_>,
  ) -> PluginRenderManifestHookOutput {
    let mut assets = vec![];
    let mut diagnostics = vec![];

    for plugin in &self.plugins {
      let res = plugin
        .render_manifest(PluginContext::new(), args.clone())
        .await?;

      let (res, diags) = res.split_into_parts();

      tracing::trace!(
        "For Chunk({:?}), Plugin({}) generate files {:?}",
        args.chunk().id,
        plugin.name(),
        res
          .iter()
          .map(|manifest| manifest.filename())
          .collect::<Vec<_>>()
      );

      assets.extend(res);
      diagnostics.extend(diags);
    }
    Ok(TWithDiagnosticArray::new(assets, diagnostics))
  }

  pub async fn render_chunk(&self, args: RenderChunkArgs<'_>) -> PluginRenderChunkHookOutput {
    for plugin in &self.plugins {
      if let Some(source) = plugin.render_chunk(PluginContext::new(), &args).await? {
        return Ok(Some(source));
      }
    }
    Ok(None)
  }

  pub fn render(&self, args: RenderArgs) -> PluginRenderHookOutput {
    for plugin in &self.plugins {
      if let Some(source) = plugin.render(PluginContext::new(), &args)? {
        return Ok(Some(source));
      }
    }
    Ok(None)
  }

  pub fn render_startup(&self, args: RenderStartupArgs) -> PluginRenderStartupHookOutput {
    let mut source = args.source;
    for plugin in &self.plugins {
      if let Some(s) = plugin.render_startup(
        PluginContext::new(),
        &RenderStartupArgs {
          source: source.clone(),
          ..args
        },
      )? {
        source = s;
      }
    }
    Ok(Some(source))
  }

  pub fn js_chunk_hash(&self, mut args: JsChunkHashArgs) -> PluginJsChunkHashHookOutput {
    for plugin in &self.plugins {
      plugin.js_chunk_hash(PluginContext::new(), &mut args)?
    }
    Ok(())
  }

  pub fn render_module_content<'a>(
    &'a self,
    mut args: RenderModuleContentArgs<'a>,
  ) -> PluginRenderModuleContentOutput<'a> {
    for plugin in &self.plugins {
      args = plugin.render_module_content(PluginContext::new(), args)?;
    }
    Ok(args)
  }

  pub async fn factorize(&self, args: &mut FactorizeArgs<'_>) -> PluginFactorizeHookOutput {
    for plugin in &self.plugins {
      if let Some(module) = plugin.factorize(PluginContext::new(), args).await? {
        return Ok(Some(module));
      }
    }
    Ok(None)
  }

  pub async fn normal_module_factory_create_module(
    &self,
    args: &mut NormalModuleCreateData<'_>,
  ) -> PluginNormalModuleFactoryCreateModuleHookOutput {
    for plugin in &self.plugins {
      tracing::trace!(
        "running normal_module_factory_create_module:{}",
        plugin.name()
      );
      if let Some(module) = plugin
        .normal_module_factory_create_module(PluginContext::new(), args)
        .await?
      {
        return Ok(Some(module));
      }
    }
    Ok(None)
  }

  pub async fn normal_module_factory_module(
    &self,
    mut module: BoxModule,
    args: &mut NormalModuleCreateData<'_>,
  ) -> PluginNormalModuleFactoryModuleHookOutput {
    for plugin in &self.plugins {
      tracing::trace!("running normal_module_factory_module:{}", plugin.name());
      module = plugin
        .normal_module_factory_module(PluginContext::new(), module, args)
        .await?;
    }
    Ok(module)
  }

  pub fn normal_module_loader(
    &self,
    loader_context: &mut LoaderContext<'_, LoaderRunnerContext>,
    module: &NormalModule,
  ) -> Result<()> {
    for plugin in &self.plugins {
      tracing::trace!("running normal_module_factory_module:{}", plugin.name());
      plugin.normal_module_loader(PluginContext::new(), loader_context, module)?;
    }
    Ok(())
  }

  pub async fn after_resolve(
    &self,
    args: &mut NormalModuleAfterResolveArgs<'_>,
  ) -> PluginNormalModuleFactoryAfterResolveOutput {
    for plugin in &self.plugins {
      tracing::trace!("running resolve for scheme:{}", plugin.name());
      if let Some(data) = plugin.after_resolve(PluginContext::new(), args).await? {
        return Ok(Some(data));
      }
    }
    Ok(None)
  }
  pub async fn context_module_before_resolve(
    &self,
    args: &mut BeforeResolveArgs,
  ) -> PluginNormalModuleFactoryBeforeResolveOutput {
    for plugin in &self.plugins {
      tracing::trace!("running resolve for scheme:{}", plugin.name());
      if let Some(data) = plugin
        .context_module_before_resolve(PluginContext::new(), args)
        .await?
      {
        return Ok(Some(data));
      }
    }
    Ok(None)
  }

  pub async fn context_module_after_resolve(
    &self,
    args: &mut NormalModuleAfterResolveArgs<'_>,
  ) -> PluginNormalModuleFactoryAfterResolveOutput {
    for plugin in &self.plugins {
      tracing::trace!("running resolve for scheme:{}", plugin.name());
      if let Some(data) = plugin
        .context_module_after_resolve(PluginContext::new(), args)
        .await?
      {
        return Ok(Some(data));
      }
    }
    Ok(None)
  }

  pub async fn normal_module_factory_resolve_for_scheme(
    &self,
    args: ResourceData,
  ) -> Result<ResourceData> {
    let mut args = args;
    for plugin in &self.plugins {
      tracing::trace!("running resolve for scheme:{}", plugin.name());
      let (ret, stop) = plugin
        .normal_module_factory_resolve_for_scheme(PluginContext::new(), args)
        .await?;
      if stop {
        return Ok(ret);
      } else {
        args = ret;
      }
    }
    Ok(args)
  }

  #[instrument(name = "plugin:additional_chunk_runtime_requirements", skip_all)]
  pub async fn additional_chunk_runtime_requirements(
    &self,
    args: &mut AdditionalChunkRuntimeRequirementsArgs<'_>,
  ) -> PluginAdditionalChunkRuntimeRequirementsOutput {
    for plugin in &self.plugins {
      plugin
        .additional_chunk_runtime_requirements(PluginContext::new(), args)
        .await?;
    }
    Ok(())
  }

  #[instrument(name = "plugin:additional_tree_runtime_requirements", skip_all)]
  pub async fn additional_tree_runtime_requirements(
    &self,
    args: &mut AdditionalChunkRuntimeRequirementsArgs<'_>,
  ) -> PluginAdditionalChunkRuntimeRequirementsOutput {
    for plugin in &self.plugins {
      plugin
        .additional_tree_runtime_requirements(PluginContext::new(), args)
        .await?;
    }
    Ok(())
  }

  pub fn runtime_requirement_in_module(
    &self,
    args: &mut AdditionalModuleRequirementsArgs,
  ) -> PluginAdditionalModuleRequirementsOutput {
    for plugin in &self.plugins {
      plugin.runtime_requirements_in_module(PluginContext::new(), args)?;
    }
    Ok(())
  }

  #[instrument(name = "plugin:runtime_requirements_in_tree", skip_all)]
  pub async fn runtime_requirements_in_tree(
    &self,
    args: &mut RuntimeRequirementsInTreeArgs<'_>,
  ) -> PluginRuntimeRequirementsInTreeOutput {
    for plugin in &self.plugins {
      plugin
        .runtime_requirements_in_tree(PluginContext::new(), args)
        .await?;
    }
    Ok(())
  }

  #[instrument(name = "plugin:after_process_assets", skip_all)]
  pub async fn after_process_assets(&self, args: ProcessAssetsArgs<'_>) -> Result<()> {
    for plugin in &self.plugins {
      plugin
        .after_process_assets(
          PluginContext::new(),
          ProcessAssetsArgs {
            compilation: args.compilation,
          },
        )
        .await?
    }
    Ok(())
  }

  #[instrument(name = "plugin:done", skip_all)]
  pub async fn done<'s, 'c>(&self, stats: &'s mut Stats<'c>) -> PluginBuildEndHookOutput {
    for plugin in &self.plugins {
      plugin
        .done(PluginContext::new(), DoneArgs { stats })
        .await?;
    }
    Ok(())
  }
  #[instrument(name = "plugin:optimize_chunks", skip_all)]
  pub async fn optimize_chunks(&self, compilation: &mut Compilation) -> Result<()> {
    for plugin in &self.plugins {
      plugin
        .optimize_chunks(PluginContext::new(), OptimizeChunksArgs { compilation })
        .await?;
    }
    Ok(())
  }

  #[instrument(name = "plugin:optimize_modules", skip_all)]
  pub async fn optimize_modules(&self, compilation: &mut Compilation) -> Result<()> {
    for plugin in &self.plugins {
      plugin.optimize_modules(compilation).await?;
    }
    Ok(())
  }

  #[instrument(name = "plugin:after_optimize_modules", skip_all)]
  pub async fn after_optimize_modules(&self, compilation: &mut Compilation) -> Result<()> {
    for plugin in &self.plugins {
      // `SyncHook`
      plugin.after_optimize_modules(compilation).await?;
    }
    Ok(())
  }

  #[instrument(name = "plugin:optimize_dependencies", skip_all)]
  pub async fn optimize_dependencies(&self, compilation: &mut Compilation) -> Result<Option<()>> {
    for plugin in &self.plugins {
      if let Some(t) = plugin.optimize_dependencies(compilation).await? {
        return Ok(Some(t));
      };
    }
    Ok(None)
  }

  #[instrument(name = "plugin:optimize_code_generation", skip_all)]
  pub async fn optimize_code_generation(
    &self,
    compilation: &mut Compilation,
  ) -> Result<Option<()>> {
    for plugin in &self.plugins {
      if let Some(t) = plugin.optimize_code_generation(compilation).await? {
        return Ok(Some(t));
      };
    }
    Ok(None)
  }

  #[instrument(name = "plugin:optimize_tree", skip_all)]
  pub async fn optimize_tree(&self, compilation: &mut Compilation) -> Result<()> {
    for plugin in &self.plugins {
      plugin.optimize_tree(compilation).await?;
    }
    Ok(())
  }

  #[instrument(name = "plugin:optimize_chunk_modules", skip_all)]
  pub async fn optimize_chunk_modules(&self, compilation: &mut Compilation) -> Result<()> {
    for plugin in &self.plugins {
      plugin
        .optimize_chunk_modules(OptimizeChunksArgs { compilation })
        .await?;
    }
    Ok(())
  }

  pub async fn resolve_loader(
    &self,
    compiler_options: &CompilerOptions,
    context: &Path,
    resolver: &Resolver,
    loader_request: &str,
    loader_options: Option<&str>,
  ) -> Result<Option<BoxLoader>> {
    for plugin in &self.plugins {
      if let Some(loader) = plugin
        .resolve_loader(
          compiler_options,
          context,
          resolver,
          loader_request,
          loader_options,
        )
        .await?
      {
        return Ok(Some(loader));
      };
    }

    Ok(None)
  }

  pub async fn before_loaders(&self, module: &mut NormalModule) -> Result<()> {
    for plugin in &self.plugins {
      plugin.before_loaders(module).await?;
    }
    Ok(())
  }

  #[instrument(name = "plugin:module_ids", skip_all)]
  pub fn module_ids(&self, compilation: &mut Compilation) -> Result<()> {
    for plugin in &self.plugins {
      plugin.module_ids(compilation)?;
    }
    Ok(())
  }

  #[instrument(name = "plugin:chunk_ids", skip_all)]
  pub fn chunk_ids(&self, compilation: &mut Compilation) -> Result<()> {
    for plugin in &self.plugins {
      plugin.chunk_ids(compilation)?;
    }
    Ok(())
  }

  #[instrument(name = "plugin:emit", skip_all)]
  pub async fn emit(&self, compilation: &mut Compilation) -> Result<()> {
    for plugin in &self.plugins {
      plugin.emit(compilation).await?;
    }
    Ok(())
  }

  #[instrument(name = "plugin:asset_emitted", skip_all)]
  pub async fn asset_emitted(&self, args: &AssetEmittedArgs<'_>) -> Result<()> {
    for plugin in &self.plugins {
      plugin.asset_emitted(args).await?;
    }
    Ok(())
  }

  #[instrument(name = "plugin:after_emit", skip_all)]
  pub async fn after_emit(&self, compilation: &mut Compilation) -> Result<()> {
    for plugin in &self.plugins {
      plugin.after_emit(compilation).await?;
    }
    Ok(())
  }

  #[instrument(name = "plugin:seal", skip_all)]
  pub fn seal(&self, compilation: &mut Compilation) -> Result<()> {
    for plugin in &self.plugins {
      plugin.seal(compilation)?;
    }
    Ok(())
  }
}
