//! The title's output size (game_scarlett_master.exe 1.0.2.1, image
//! offsets). Its Series S output of 1920x1080, and the 16:9 its projection
//! is made with, come from three places:
//!
//! - the display object's constructor (+0x5eb1c60, the object at
//!   +0xa9437b8) copies `{100, 1920, 1080, 60}` from +0x615bf70 to its
//!   +0x110: the width and height (+0xa9438cc, +0xa9438d0) the window
//!   (+0x24784ec), the presented planes and the renderer are made of;
//! - the same constructor puts 16:9 (`mov r10, 3FE38E39_00000000h`, +0x5eb1c7e)
//!   in the object's +0x1a8 (+0xa943960), the aspect the projection uses;
//! - the settings' default (+0x5b37a20: `mov rax, 1080 << 32 | 1920 ; mov
//!   [rcx+204h], rax`). Applying the settings (+0x22434a0) copies them to the
//!   display object (the setter +0x2478130) and makes the aspect width /
//!   height again, so all three are given the same size.
//!
//! With `VICTORY_DISPLAY_MODE=WxH` the title is given that size instead, as
//! if it rendered for the display it is told of.
//!
//! Left as they are: places that fit buffers into 1920x1080 keeping their
//! proportions (+0x2627f10, +0x5bfa6e0), which lower those buffers'
//! resolution at a wider size without distorting them.

use victory_loader::frame_rate::{Patched, write_checked};

/// Image offset, the bytes there, what goes there for `width` x `height`.
type Site = (usize, &'static [u8], fn(u32, u32) -> Vec<u8>);

const SITES: [Site; 3] = [
    // .rdata: the constructor's width and height
    (0x615bf74, &[0x80, 0x07, 0, 0, 0x38, 0x04, 0, 0], |w, h| [w.to_le_bytes(), h.to_le_bytes()].concat()),
    // mov r10, aspect << 32
    (0x5eb1c7e, &[0x49, 0xba, 0, 0, 0, 0, 0x39, 0x8e, 0xe3, 0x3f], |w, h| {
        [&[0x49, 0xba, 0, 0, 0, 0][..], &(w as f32 / h as f32).to_le_bytes()].concat()
    }),
    // mov rax, height << 32 | width (then mov [rcx+204h], rax ; ret)
    (
        0x5b37a20,
        &[0x48, 0xb8, 0x80, 0x07, 0, 0, 0x38, 0x04, 0, 0, 0x48, 0x89, 0x81, 0x04, 0x02, 0, 0, 0xc3],
        |w, h| [&[0x48, 0xb8][..], &w.to_le_bytes(), &h.to_le_bytes()].concat(),
    ),
];

/// Gives the title `VICTORY_DISPLAY_MODE`'s size, if set: every site or,
/// if any differs (another build), none.
///
/// # Safety
/// The image of `size` bytes at `base` stays mapped and none of its code
/// has run.
pub unsafe fn install(base: usize, size: usize) {
    let Some((width, height)) = victory_gpu::dxgi::asked_display_mode() else {
        return;
    };
    // All or nothing: a size half given would be undone by the settings.
    let differs = SITES.iter().find(|(at, expected, _)| {
        at + expected.len() > size
            // SAFETY: inside the mapped image (checked first).
            || unsafe { std::slice::from_raw_parts((base + at) as *const u8, expected.len()) } != *expected
    });
    if let Some((at, ..)) = differs {
        tracing::warn!(site = format_args!("{at:#x}"), "title output size: not patched, the title's bytes are not the ones expected (another build?)");
        return;
    }
    for (at, expected, code) in SITES {
        // SAFETY: the caller's promise.
        if let Patched::Failed(why) = unsafe { write_checked(base, size, at, expected, &code(width, height)) } {
            tracing::warn!(site = format_args!("{at:#x}"), why, "title output size: not patched");
        }
    }
    tracing::info!(width, height, aspect = width as f32 / height as f32, "title output size: its 1920x1080 and 16:9 replaced (VICTORY_DISPLAY_MODE)");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_titles_own_size_encodes_as_its_own_bytes() {
        for (at, expected, code) in SITES {
            let own = code(1920, 1080);
            assert_eq!(own, expected[..own.len()], "site {at:#x}");
        }
        assert_eq!(SITES[0].2(2752, 1152), [0xc0, 0x0a, 0, 0, 0x80, 0x04, 0, 0]);
    }
}
