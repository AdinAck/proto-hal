use std::{ops::Deref, str::FromStr};

#[derive(Debug, Clone)]
pub struct NumericValue(u32);

impl Deref for NumericValue {
    type Target = u32;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl FromStr for NumericValue {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some(addr) = s.strip_prefix("0x") {
            Ok(Self(
                u32::from_str_radix(addr, 16).map_err(|e| e.to_string())?,
            ))
        } else {
            Ok(Self(u32::from_str(s).map_err(|e| e.to_string())?))
        }
    }
}
