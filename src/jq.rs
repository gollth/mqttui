use std::iter::empty;

use color_eyre::{Result, eyre::eyre};
use enum_as_inner::EnumAsInner;
use jaq_core::{
    Compiler, Ctx, RcIter,
    load::{Arena, File, Loader},
};
use jaq_json::Val;
use serde_json::Value;

#[derive(Debug, Default, Clone, EnumAsInner)]
pub enum Jaqqer {
    #[default]
    Dormant,
    Prompt(String),
    Active(String),
}

impl Jaqqer {
    /// Put this JQ filter in edit mode, if it isn't already
    pub(crate) fn edit(self) -> Self {
        match self {
            Self::Dormant => Self::Prompt(Default::default()),
            Self::Prompt(p) => Self::Prompt(p),
            Self::Active(p) => Self::Prompt(p),
        }
    }

    /// Put this JQ filter in active mode, if it isn't already
    pub(crate) fn activate(self) -> Jaqqer {
        match self {
            Self::Dormant => Self::Dormant,
            Self::Prompt(p) => Self::Active(p),
            Self::Active(p) => Self::Active(p),
        }
    }

    /// Rest this JQ filter and put it back in dormant mode
    pub(crate) fn clear(self) -> Jaqqer {
        match self {
            Self::Dormant => Self::Dormant,
            Self::Prompt(_) => Self::Dormant,
            Self::Active(_) => Self::Dormant,
        }
    }

    pub(crate) fn run(&self, value: Value) -> Result<Vec<Value>> {
        let Some(code) = self.as_prompt().or(self.as_active()).map(|p| p.as_str()) else {
            return Ok(Vec::new());
        };
        let loader = Loader::new(jaq_std::defs().chain(jaq_json::defs()));
        let arena = Arena::default();

        let program = File { code, path: () };
        let modules = loader.load(&arena, program).map_err(|e| eyre!("{e:?}"))?;

        let filter = Compiler::default()
            .with_funs(jaq_std::funs().chain(jaq_json::funs()))
            .compile(modules)
            .map_err(|e| eyre!("{e:?}"))?;
        let input = RcIter::new(empty());
        Ok(filter
            .run((Ctx::new([], &input), Val::from(value)))
            .filter_map(|value| value.ok())
            .map(Value::from)
            .collect())
    }
}
