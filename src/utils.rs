use rand::Rng;

pub fn generate_password<const N: usize>() -> String {
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ\
                            abcdefghijklmnopqrstuvwxyz\
                            0123456789)(*&^%#@!~";
    let mut rng = rand::rng();
    (0..N)
        .map(|_| {
            let idx = rng.random_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}

pub mod filters {
    use std::path::PathBuf;

    pub fn path(p: &PathBuf) -> rinja::Result<String> {
        Ok(p.canonicalize()
            .map_err(|e| {
                rinja::Error::Custom(
                    anyhow::Error::from(e)
                        .context(format!("Failed to canonicalize {p:?}"))
                        .into(),
                )
            })?
            .display()
            .to_string())
    }
}
