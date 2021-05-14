use anyhow::Result;
use enigo::*;

use crate::i3_config::{ALT_PATTERN, CONTROL_PATTERN, META_PATTERN, SHIFT_PATTERN};

pub fn execute(key_sequence: &str) -> Result<()> {
    let mut buffer = key_sequence.to_lowercase();

    // this was just a test
    // TODO (David): build a hashmap of keys to key types
    let mut alt_used = false;
    let mut ctrl_used = false;
    let mut meta_used = false;
    let mut shift_used = false;

    if buffer.contains(ALT_PATTERN) {
        buffer = buffer.replace(ALT_PATTERN, "");
        alt_used = true;
    }
    if buffer.contains(CONTROL_PATTERN) {
        buffer = buffer.replace(CONTROL_PATTERN, "");
        ctrl_used = true;
    }
    if buffer.contains(META_PATTERN) {
        buffer = buffer.replace(META_PATTERN, "");
        meta_used = true;
    }
    if buffer.contains(SHIFT_PATTERN) {
        buffer = buffer.replace(SHIFT_PATTERN, "");
        shift_used = true;
    }

    buffer = buffer.trim().to_lowercase();

    if buffer
        .chars()
        .all(|character| character.is_ascii_alphabetic())
    {
        let mut enigo = enigo::Enigo::new();

        if alt_used {
            enigo.key_down(enigo::Key::Alt);
        }
        if ctrl_used {
            enigo.key_down(enigo::Key::Control);
        }
        if meta_used {
            enigo.key_down(enigo::Key::Meta);
        }
        if shift_used {
            enigo.key_down(enigo::Key::Shift);
        }

        enigo.key_sequence(&buffer);

        if alt_used {
            enigo.key_up(enigo::Key::Alt);
        }
        if ctrl_used {
            enigo.key_up(enigo::Key::Control);
        }
        if meta_used {
            enigo.key_up(enigo::Key::Meta);
        }
        if shift_used {
            enigo.key_up(enigo::Key::Shift);
        }
    } else {
        return Err(anyhow::anyhow!("Keys aren't alphanumeric"));
    }

    // enigo.key_down(enigo::Key::Meta);
    // enigo.key_sequence("d");
    // enigo.key_up(enigo::Key::Meta);
    Ok(())
}
