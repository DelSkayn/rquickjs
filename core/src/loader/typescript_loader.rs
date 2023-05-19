use super::check_extensions;
use crate::{Ctx, Error, Loaded, Loader, Module, Result, Script};
use derivative::Derivative;
use std::io::stderr;
use swc_core::{
    base::{config::IsModule, Compiler},
    common::{errors::Handler, sync::Lrc, FileName, Globals, Mark, SourceFile, SourceMap, GLOBALS},
    ecma::{
        ast::EsVersion,
        codegen::{text_writer::JsWriter, Emitter},
        parser::Syntax,
        transforms::{
            base::{fixer::fixer, hygiene::hygiene, resolver},
            typescript::strip,
        },
        visit::FoldWith,
    },
};

/// The script module loader
///
/// This loader can be used as the nested backing loader in user-defined loaders.
#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "loader")))]
#[derive(Derivative)]
#[derivative(Debug)]
pub struct TypeScriptLoader {
    extensions: Vec<String>,
    #[derivative(Debug = "ignore")]
    transpiler: EasySwcTranspiler,
    syntax: Syntax,
    is_module: IsModule,
}

impl TypeScriptLoader {
    /// Add script file extension
    pub fn add_extension<X: Into<String>>(&mut self, extension: X) -> &mut Self {
        self.extensions.push(extension.into());
        self
    }

    /// Add script file extension
    #[must_use]
    pub fn with_extension<X: Into<String>>(mut self, extension: X) -> Self {
        self.add_extension(extension);
        self
    }
}

impl Default for TypeScriptLoader {
    fn default() -> Self {
        Self {
            extensions: vec!["js".into(), "ts".into()],
            transpiler: EasySwcTranspiler::default(),
            syntax: Syntax::Typescript(Default::default()),
            is_module: IsModule::Unknown,
        }
    }
}

impl Loader<Script> for TypeScriptLoader {
    fn load<'js>(&mut self, ctx: Ctx<'js>, path: &str) -> Result<Module<'js, Loaded<Script>>> {
        if !check_extensions(path, &self.extensions) {
            return Err(Error::new_loading(path));
        }

        let source = self
            .transpiler
            .transpile(
                FileName::Real(path.to_string().into()),
                std::fs::read_to_string(path)?,
                self.syntax,
                self.is_module,
            )
            .map_err(|e| Error::new_loading_message(path, format!("transpiler error: {e}")))?;
        Module::new(ctx, path, source)
    }
}

pub struct EasySwcTranspiler {
    source_map: Lrc<SourceMap>,
    compiler: Compiler,
    handler: Handler,
    globals: Globals,
}

impl Default for EasySwcTranspiler {
    fn default() -> Self {
        let source_map: Lrc<SourceMap> = Default::default();
        let compiler = Compiler::new(source_map.clone());
        let handler = Handler::with_emitter_writer(Box::new(stderr()), Some(compiler.cm.clone()));
        let globals = Globals::new();

        Self {
            source_map,
            compiler,
            handler,
            globals,
        }
    }
}

impl EasySwcTranspiler {
    pub fn transpile(
        &self,
        filename: FileName,
        source: String,
        syntax: Syntax,
        is_module: IsModule,
    ) -> anyhow::Result<Vec<u8>> {
        let fm = self
            .source_map
            .new_source_file_from(filename, source.into());

        GLOBALS.set(&self.globals, || self.do_transpile(fm, syntax, is_module))
    }

    fn do_transpile(
        &self,
        fm: Lrc<SourceFile>,
        syntax: Syntax,
        is_module: IsModule,
    ) -> anyhow::Result<Vec<u8>> {
        let mut program = self.compiler.parse_js(
            fm,
            &self.handler,
            EsVersion::Es2022,
            syntax,
            is_module,
            None,
        )?;

        let unresolved_mark = Mark::new();
        let top_level_mark = Mark::new();

        program = match syntax {
            Syntax::Typescript(_) => program
                .fold_with(&mut resolver(unresolved_mark, top_level_mark, true))
                .fold_with(&mut strip(top_level_mark)),
            Syntax::Es(_) => {
                program.fold_with(&mut resolver(unresolved_mark, top_level_mark, false))
            }
        };

        program = program
            .fold_with(&mut hygiene())
            .fold_with(&mut fixer(None));

        let mut buf = vec![];
        let wr = JsWriter::new(self.source_map.clone(), "\n", &mut buf, None);
        let cfg = swc_core::ecma::codegen::Config {
            target: EsVersion::Es2020,
            ..Default::default()
        };
        let mut emitter = Emitter {
            cfg,
            cm: self.source_map.clone(),
            comments: None,
            wr,
        };

        emitter.emit_program(&program)?;

        Ok(buf.into())
    }
}
