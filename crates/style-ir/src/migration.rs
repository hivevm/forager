//! The migration of a style document of an older major to the current one, one major at a time,
//! on its value tree before it is read as the current version (ADR-0020).

use serde_json::Value;

use crate::Diagnostic;

/// A step from the format version `from_major` to the next major, on the document's value tree.
/// Its error says what in the document it could not migrate.
#[derive(Clone, Copy)]
pub struct Step {
    pub from_major: u32,
    pub migrate: fn(Value) -> Result<Value, String>,
}

/// The steps this build migrates with. Major 1 is the first, so there is none yet.
pub const STEPS: &[Step] = &[];

/// `value`, of format version `version` with the major `from`, migrated by `steps` to the major
/// `to`. Each step leaves the document at the next major's version `<major>.0`.
pub(crate) fn migrate(
    mut value: Value,
    version: &str,
    from: u32,
    to: u32,
    steps: &[Step],
) -> Result<Value, Diagnostic> {
    for major in from..to {
        let Some(step) = steps.iter().find(|s| s.from_major == major) else {
            let message = format!(
                "format version {version} is older than this build migrates from; it reads {} \
                 and has no migration from major {major}",
                crate::FORMAT_VERSION
            );
            return Err(Diagnostic::error(
                "format-version-unsupported",
                message,
                None,
            ));
        };
        value = (step.migrate)(value).map_err(|reason| {
            let message =
                format!("format version {version} cannot be migrated from major {major}: {reason}");
            Diagnostic::error("format-migration-failed", message, None)
        })?;
        if let Some(document) = value.as_object_mut() {
            document.insert("version".to_owned(), format!("{}.0", major + 1).into());
        }
    }
    Ok(value)
}
