//! "Instalar o recomendado": what to download on a new device and how big it is.

use serde::Serialize;

/// Local model for this device: E4B with 16 GB of RAM or more (some is always
/// reserved by the system, hence the lower threshold), E2B below that and on Android.
pub fn pick_model(total_ram_bytes: u64, android: bool) -> &'static str {
    // 16 GiB minus ~5%.
    if !android && total_ram_bytes >= 15_200_000_000 {
        "gemma-4-e4b"
    } else {
        "gemma-4-e2b"
    }
}

/// Total RAM of this device in bytes (0 when it can't be read).
pub fn total_ram() -> u64 {
    let Ok(mut sys) = sysinfo::System::new() else { return 0 };
    sys.refresh_memory();
    sys.total_memory()
}

/// Download sizes shown in the first-run screen (the corpus and the
/// dictionary aren't fixed files, so these are the usual sizes).
pub const TATOEBA_BYTES: u64 = 36_000_000;
pub const DICT_BYTES: u64 = 12_000_000;

#[derive(Debug, Clone, Serialize)]
pub struct SetupItem {
    /// "tatoeba" | "dict" | "voice" | "model"
    pub id: String,
    pub label: String,
    pub bytes: u64,
    pub done: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct RecommendedSetup {
    pub model_id: String,
    pub model_name: String,
    pub items: Vec<SetupItem>,
    /// Bytes still to download (items not done).
    pub total_bytes: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_with_16gb_gets_e4b() {
        assert_eq!(pick_model(16 * (1 << 30), false), "gemma-4-e4b");
    }

    #[test]
    fn reserved_ram_still_counts_as_16gb() {
        assert_eq!(pick_model(15_500_000_000, false), "gemma-4-e4b");
    }

    #[test]
    fn small_pc_gets_e2b() {
        assert_eq!(pick_model(8 * (1 << 30), false), "gemma-4-e2b");
    }

    #[test]
    fn android_always_e2b() {
        assert_eq!(pick_model(24 * (1 << 30), true), "gemma-4-e2b");
    }

    #[test]
    fn reads_some_ram() {
        assert!(total_ram() > 0);
    }

    #[test]
    fn picked_models_exist_in_the_catalog() {
        for id in [pick_model(32 * (1 << 30), false), pick_model(0, false)] {
            assert!(local_llm::find(id).is_some(), "{id}");
        }
    }
}
