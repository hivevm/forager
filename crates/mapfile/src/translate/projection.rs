//! A `PROJECTION` block as a proj4 string, its `init=` reference resolved by an init file the
//! mapfile names or by the host's projection resolver (ADR-0023).

use forager_style_ir::LayerProjection;

use super::Translator;
use crate::projection::{self, Init, Projection};
use crate::syntax::Block;

impl Translator<'_> {
    /// A proj4 string from the parameters of a PROJECTION block (`projection::read`), its
    /// `init=` reference resolved and expanded after the explicit parameters, which PROJ lets
    /// take precedence (its `pj_expand_init`). Without a resolver, or for a code it does not
    /// know, an `init=` is a load error (ADR-0023).
    pub(super) fn projection(&mut self, block: &Block) -> Option<LayerProjection> {
        let (mut params, init) = match projection::read(block) {
            Ok(Projection::Auto) => return Some(LayerProjection::FromSource),
            Ok(Projection::Parameters { explicit, init }) => (explicit, init),
            Err(invalid) => {
                self.error("invalid-value", invalid.message, &invalid.span);
                return None;
            }
        };
        if let Some(init) = init {
            let resolved = match self.resolve(&init) {
                Ok(resolved) => resolved,
                Err(why) => {
                    let message = format!("`{}`: {why}", init.param);
                    self.error("projection-unresolved", &message, &init.span);
                    return None;
                }
            };
            params.extend(
                resolved
                    .split_whitespace()
                    .map(|p| format!("+{}", p.trim_start_matches('+'))),
            );
        }
        (!params.is_empty()).then(|| LayerProjection::Proj4(params.join(" ")))
    }

    /// The definition an `init=` reference names: from the init file the mapfile names when its
    /// file name holds a `/`, else from the host's resolver, with the file name as the authority
    /// (ADR-0023).
    fn resolve(&self, init: &Init) -> Result<String, String> {
        let Some((file, code)) = &init.reference else {
            return Err("a projection is named as `<authority>:<code>`".to_owned());
        };
        if let Some(file) = init.file() {
            let init_file = self
                .init_files
                .get(file)
                .ok_or_else(|| format!("the init file `{file}` was not read"))?;
            return match init_file {
                Ok(init_file) => init_file.definition(code).map(str::to_owned),
                Err(why) => Err(why.clone()),
            };
        }
        match self.resolver {
            Some(resolver) => resolver
                .resolve(file, code)
                .ok_or_else(|| "it is not a projection the resolver knows".to_owned()),
            None => Err("it needs a projection resolver, and none is given".to_owned()),
        }
    }
}
