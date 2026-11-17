//! A plugin's own text catalogs: `locales/<tag>.ftl`, read with the rest of its bundle.

use model::Files;

use crate::bundle::Bundle;

/// The bundle's catalogs (the bundle already skipped any that broke a rule).
pub fn load(bundle: &Bundle) -> Files {
    bundle.catalogs().iter().map(|(tag, source)| (tag.as_str().into(), source.as_str().into())).collect()
}
