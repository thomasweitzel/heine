//! Validation of the fixed keyword contracts for Heine template built-ins.

/// Rejects misspelled or unsupported keyword arguments instead of silently
/// ignoring them. Translation arguments are intentionally handled separately:
/// their names belong to the authored Fluent message contract.
pub(super) fn only(
    function: &str,
    kwargs: &tera::Kwargs,
    allowed: &[&str],
) -> tera::TeraResult<()> {
    let unexpected = kwargs.iter().find_map(|(key, _)| match key.as_str() {
        Some(supplied) if allowed.contains(&supplied) => None,
        Some(supplied) => Some(supplied.to_owned()),
        None => Some(key.to_string()),
    });
    let Some(unexpected) = unexpected else {
        return Ok(());
    };
    let allowed = match allowed {
        [] => "no keyword arguments".to_owned(),
        [one] => format!("only {one:?}"),
        _ => format!(
            "only {}",
            allowed
                .iter()
                .map(|name| format!("{name:?}"))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    };
    Err(tera::Error::message(format!(
        "{function}() does not accept keyword argument {unexpected:?}; it accepts {allowed}"
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_declared_arguments_and_rejects_other_names() {
        let accepted = tera::Kwargs::from([("id", tera::Value::from("guide"))]);
        assert!(only("page", &accepted, &["id", "locale"]).is_ok());

        let unexpected = tera::Kwargs::from([("local", tera::Value::from("de"))]);
        let error =
            only("page", &unexpected, &["id", "locale"]).expect_err("unknown keyword must fail");
        assert!(error.to_string().contains("local"));
    }
}
