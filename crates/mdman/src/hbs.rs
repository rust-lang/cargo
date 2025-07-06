//! Handlebars template processing.

use std::collections::HashMap;
use std::path::Path;

use anyhow::Error;
use handlebars::{
    Context, Decorator, DirectorySourceOptions, Handlebars, Helper, HelperDef, HelperResult,
    Output, RenderContext, RenderError, RenderErrorReason, Renderable, handlebars_helper,
};

use crate::format::Formatter;

type FormatterRef<'a> = &'a (dyn Formatter + Send + Sync);

/// Processes the handlebars template at the given file.
pub fn expand(file: &Path, formatter: FormatterRef<'_>) -> Result<String, Error> {
    let mut handlebars = Handlebars::new();
    handlebars.set_strict_mode(true);
    handlebars.register_helper("lower", Box::new(lower));
    handlebars.register_helper("options", Box::new(OptionsHelper { formatter }));
    handlebars.register_helper("option", Box::new(OptionHelper { formatter }));
    handlebars.register_helper("man", Box::new(ManLinkHelper { formatter }));
    handlebars.register_decorator("set", Box::new(set_decorator));
    handlebars.register_template_file("template", file)?;
    let includes = file.parent().unwrap().join("includes");
    let mut options = DirectorySourceOptions::default();
    options.tpl_extension = ".md".to_string();
    handlebars.register_templates_directory(includes, options)?;
    let man_name = file
        .file_stem()
        .expect("expected filename")
        .to_str()
        .expect("utf8 filename")
        .to_string();
    let data = HashMap::from([("man_name", man_name)]);
    let expanded = handlebars.render("template", &data)?;
    Ok(expanded)
}

/// Helper for `{{#options}}` block.
struct OptionsHelper<'a> {
    formatter: FormatterRef<'a>,
}

impl HelperDef for OptionsHelper<'_> {
    fn call<'reg: 'rc, 'rc>(
        &self,
        h: &Helper<'rc>,
        r: &'reg Handlebars<'reg>,
        ctx: &'rc Context,
        rc: &mut RenderContext<'reg, 'rc>,
        out: &mut dyn Output,
    ) -> HelperResult {
        if in_options(rc) {
            return Err(
                RenderErrorReason::Other("options blocks cannot be nested".to_string()).into(),
            );
        }
        // Prevent nested {{#options}}.
        set_in_context(rc, "__MDMAN_IN_OPTIONS", serde_json::Value::Bool(true));
        let s = self.formatter.render_options_start();
        out.write(&s)?;
        let t = match h.template() {
            Some(t) => t,
            None => {
                return Err(RenderErrorReason::Other(
                    "options block must not be empty".to_string(),
                )
                .into());
            }
        };
        let block = t.renders(r, ctx, rc)?;
        out.write(&block)?;

        let s = self.formatter.render_options_end();
        out.write(&s)?;
        remove_from_context(rc, "__MDMAN_IN_OPTIONS");
        Ok(())
    }
}

/// Whether or not the context is currently inside a `{{#options}}` block.
fn in_options(rc: &RenderContext<'_, '_>) -> bool {
    rc.context()
        .map_or(false, |ctx| ctx.data().get("__MDMAN_IN_OPTIONS").is_some())
}

/// Helper for `{{#option}}` block.
struct OptionHelper<'a> {
    formatter: FormatterRef<'a>,
}

impl HelperDef for OptionHelper<'_> {
    fn call<'reg: 'rc, 'rc>(
        &self,
        h: &Helper<'rc>,
        r: &'reg Handlebars<'reg>,
        gctx: &'rc Context,
        rc: &mut RenderContext<'reg, 'rc>,
        out: &mut dyn Output,
    ) -> HelperResult {
        if !in_options(rc) {
            return Err(
                RenderErrorReason::Other("option must be in options block".to_string()).into(),
            );
        }
        let params = h.params();
        if params.is_empty() {
            return Err(RenderErrorReason::Other(
                "option block must have at least one param".to_string(),
            )
            .into());
        }
        // Convert params to strings.
        let params = params
            .iter()
            .map(|param| {
                param
                    .value()
                    .as_str()
                    .ok_or_else(|| {
                        RenderErrorReason::Other("option params must be strings".to_string())
                    })
                    .into()
            })
            .collect::<Result<Vec<&str>, RenderErrorReason>>()?;
        let t = match h.template() {
            Some(t) => t,
            None => {
                return Err(
                    RenderErrorReason::Other("option block must not be empty".to_string()).into(),
                );
            }
        };
        // Render the block.
        let block = t.renders(r, gctx, rc)?;

        // Windows newlines can break some rendering, so normalize.
        let block = block.replace("\r\n", "\n");

        // Get the name of this page.
        let man_name = gctx
            .data()
            .get("man_name")
            .expect("expected man_name in context")
            .as_str()
            .expect("expect man_name str");

        // Ask the formatter to convert this option to its format.
        let option = self
            .formatter
            .render_option(&params, &block, man_name)
            .map_err(|e| RenderErrorReason::Other(format!("option render failed: {}", e)))?;
        out.write(&option)?;
        Ok(())
    }
}

/// Helper for `{{man name section}}` expression.
struct ManLinkHelper<'a> {
    formatter: FormatterRef<'a>,
}

impl HelperDef for ManLinkHelper<'_> {
    fn call<'reg: 'rc, 'rc>(
        &self,
        h: &Helper<'rc>,
        _r: &'reg Handlebars<'reg>,
        _ctx: &'rc Context,
        _rc: &mut RenderContext<'reg, 'rc>,
        out: &mut dyn Output,
    ) -> HelperResult {
        let params = h.params();
        if params.len() != 2 {
            return Err(
                RenderErrorReason::Other("{{man}} must have two arguments".to_string()).into(),
            );
        }
        let name = params[0].value().as_str().ok_or_else(|| {
            RenderErrorReason::Other("man link name must be a string".to_string())
        })?;
        let section = params[1].value().as_u64().ok_or_else(|| {
            RenderErrorReason::Other("man link section must be an integer".to_string())
        })?;
        let section = u8::try_from(section)
            .map_err(|_e| RenderErrorReason::Other("section number too large".to_string()))?;
        let link = self
            .formatter
            .linkify_man_to_md(name, section)
            .map_err(|e| RenderErrorReason::Other(format!("failed to linkify man: {}", e)))?;
        out.write(&link)?;
        Ok(())
    }
}

/// `{{*set var=value}}` decorator.
///
/// This sets a variable to a value within the template context.
fn set_decorator(
    d: &Decorator<'_>,
    _: &Handlebars<'_>,
    _ctx: &Context,
    rc: &mut RenderContext<'_, '_>,
) -> Result<(), RenderError> {
    let data_to_set = d.hash();
    for (k, v) in data_to_set {
        set_in_context(rc, k, v.value().clone());
    }
    Ok(())
}

/// Sets a variable to a value within the context.
fn set_in_context(rc: &mut RenderContext<'_, '_>, key: &str, value: serde_json::Value) {
    let mut gctx = match rc.context() {
        Some(c) => (*c).clone(),
        None => Context::wraps(serde_json::Value::Object(serde_json::Map::new())).unwrap(),
    };
    if let serde_json::Value::Object(m) = gctx.data_mut() {
        m.insert(key.to_string(), value);
        rc.set_context(gctx);
    } else {
        panic!("expected object in context");
    }
}

/// Removes a variable from the context.
fn remove_from_context(rc: &mut RenderContext<'_, '_>, key: &str) {
    let gctx = rc.context().expect("cannot remove from null context");
    let mut gctx = (*gctx).clone();
    if let serde_json::Value::Object(m) = gctx.data_mut() {
        m.remove(key);
        rc.set_context(gctx);
    } else {
        panic!("expected object in context");
    }
}

handlebars_helper!(lower: |s: str| s.to_lowercase());
