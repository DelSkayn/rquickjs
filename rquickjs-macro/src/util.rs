#[cfg(not(test))]
pub(crate) use proc_macro_crate::crate_name;

#[cfg(test)]
pub(crate) fn crate_name(name: &str) -> Result<String, String> {
    Ok(name.replace('-', "_"))
}
