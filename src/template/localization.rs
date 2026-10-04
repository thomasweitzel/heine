//! Locale-aware template built-ins.

use crate::localization::{
    Arguments,
    number::{self, LocalizedNumber, Options as NumberOptions},
    temporal::{self, Style},
};

use super::{arguments, runtime::TemplateRuntime};

pub(super) fn register(tera: &mut tera::Tera, runtime: &TemplateRuntime) {
    let translation_runtime = runtime.clone();
    tera.register_function(
        "t",
        move |kwargs: tera::Kwargs, state: &tera::State| -> tera::TeraResult<tera::Value> {
            let id = kwargs.must_get::<&str>("id")?;
            let attribute = kwargs.get::<&str>("attribute")?;
            let mut arguments = Arguments::new();
            for (key, value) in kwargs.iter() {
                let Some(key) = key.as_str() else {
                    return Err(tera::Error::message(
                        "translation argument names must be strings",
                    ));
                };
                if matches!(key, "id" | "attribute") {
                    continue;
                }
                if value.is_i64() || value.is_u64() || value.is_f64() {
                    arguments.set_number(key.to_owned(), localized_number(value)?);
                } else if let Some(value) = value.as_bool() {
                    arguments.set(key.to_owned(), value.to_string());
                } else if let Some(value) = value.as_str() {
                    arguments.set(key.to_owned(), value.to_owned());
                } else {
                    return Err(tera::Error::message(format!(
                        "translation argument {key:?} must be a string, number, or boolean"
                    )));
                }
            }
            translation_runtime
                .localization()
                .format(
                    &translation_runtime.locale(state)?,
                    id,
                    attribute,
                    &arguments,
                )
                .map(Into::into)
                .map_err(tera::Error::message)
        },
    );

    let date_runtime = runtime.clone();
    tera.register_function(
        "format_date",
        move |kwargs: tera::Kwargs, state: &tera::State| -> tera::TeraResult<tera::Value> {
            arguments::only("format_date", &kwargs, &["value", "style"])?;
            let value = kwargs.must_get::<&str>("value")?;
            let style = kwargs.get::<&str>("style")?.unwrap_or("medium");
            let locale = date_runtime.locale(state)?;
            temporal::format_date(
                &locale,
                date_runtime.editorial_time_zone(),
                value,
                Style::parse(style, "date style").map_err(tera::Error::message)?,
            )
            .map(Into::into)
            .map_err(tera::Error::message)
        },
    );

    let time_runtime = runtime.clone();
    tera.register_function(
        "format_time",
        move |kwargs: tera::Kwargs, state: &tera::State| -> tera::TeraResult<tera::Value> {
            arguments::only("format_time", &kwargs, &["value", "style"])?;
            let value = kwargs.must_get::<&str>("value")?;
            let style = kwargs.get::<&str>("style")?.unwrap_or("medium");
            temporal::format_time(
                &time_runtime.locale(state)?,
                time_runtime.editorial_time_zone(),
                value,
                Style::parse(style, "time style").map_err(tera::Error::message)?,
            )
            .map(Into::into)
            .map_err(tera::Error::message)
        },
    );

    let datetime_runtime = runtime.clone();
    tera.register_function(
        "format_datetime",
        move |kwargs: tera::Kwargs, state: &tera::State| -> tera::TeraResult<tera::Value> {
            arguments::only(
                "format_datetime",
                &kwargs,
                &["value", "date_style", "time_style"],
            )?;
            let value = kwargs.must_get::<&str>("value")?;
            let date_style = kwargs.get::<&str>("date_style")?.unwrap_or("medium");
            let time_style = kwargs.get::<&str>("time_style")?.unwrap_or("medium");
            temporal::format_datetime(
                &datetime_runtime.locale(state)?,
                datetime_runtime.editorial_time_zone(),
                value,
                Style::parse(date_style, "date_style").map_err(tera::Error::message)?,
                Style::parse(time_style, "time_style").map_err(tera::Error::message)?,
            )
            .map(Into::into)
            .map_err(tera::Error::message)
        },
    );

    let number_runtime = runtime.clone();
    tera.register_function(
        "format_number",
        move |kwargs: tera::Kwargs, state: &tera::State| -> tera::TeraResult<tera::Value> {
            arguments::only(
                "format_number",
                &kwargs,
                &[
                    "value",
                    "grouping",
                    "minimum_fraction_digits",
                    "maximum_fraction_digits",
                ],
            )?;
            let value = kwargs.get("value")?.ok_or_else(|| {
                tera::Error::message("format_number() requires keyword argument \"value\"")
            })?;
            let number = localized_number(value)?;
            let grouping = kwargs.get::<bool>("grouping")?.unwrap_or(true);
            let minimum = fraction_digits(&kwargs, "minimum_fraction_digits")?.unwrap_or(0);
            let maximum = fraction_digits(&kwargs, "maximum_fraction_digits")?;
            let options = NumberOptions::with_optional_maximum(grouping, minimum, maximum)
                .map_err(|error| tera::Error::message(error.to_string()))?;
            number::format(&number_runtime.locale(state)?, &number, options)
                .map(Into::into)
                .map_err(|error| tera::Error::message(error.to_string()))
        },
    );
}
fn localized_number(value: &tera::Value) -> tera::TeraResult<LocalizedNumber> {
    if let Some(value) = value.as_i64() {
        Ok(LocalizedNumber::from_i64(value))
    } else if let Some(value) = value.as_u64() {
        Ok(LocalizedNumber::from_u64(value))
    } else if let Some(value) = value.as_f64() {
        LocalizedNumber::from_f64(value).map_err(|error| tera::Error::message(error.to_string()))
    } else {
        Err(tera::Error::message(
            "format_number() value must be a finite number",
        ))
    }
}

fn fraction_digits(kwargs: &tera::Kwargs, name: &'static str) -> tera::TeraResult<Option<u16>> {
    match kwargs.get::<u64>(name)? {
        Some(value) => u16::try_from(value).map(Some).map_err(|_| {
            tera::Error::message(format!("{name} must be an integer from 0 through 32768"))
        }),
        None => Ok(None),
    }
}
