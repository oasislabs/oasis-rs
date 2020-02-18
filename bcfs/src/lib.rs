type Result<T> = std::result::Result<T, wasi_types::ErrNo>;

mod bcfs;
mod file;

pub use crate::bcfs::BCFS;

#[cfg(test)]
mod tests;
