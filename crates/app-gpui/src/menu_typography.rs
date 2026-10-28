//! Bravura runs in native menu titles. GPUI keeps the plain title and action;
//! AppKit receives an attributed display title after each menu rebuild.
use gpui::{App, Menu};

pub fn install(menus: Vec<Menu>, cx: &mut App) {
    #[cfg(target_os = "macos")]
    let titles = titles::collect(&menus);
    cx.set_menus(menus);
    #[cfg(target_os = "macos")]
    native::apply(&titles);
}

#[cfg(target_os = "macos")]
mod native {
    #![allow(deprecated, unexpected_cfgs)]
    use super::titles::{Entry, Title};
    use crate::music_font::MusicFont;
    use block::ConcreteBlock;
    use cocoa::{
        base::{BOOL, NO, YES, id, nil},
        foundation::{NSAutoreleasePool, NSPoint, NSRange, NSRect, NSSize, NSString},
    };
    use core_graphics::{
        context::CGContext,
        geometry::{CGPoint, CGSize},
    };
    use core_text::font::{self, CTFont};
    use objc::{class, msg_send, rc::WeakPtr, sel, sel_impl};

    const MUSIC_SIZE: f32 = 24.;

    #[link(name = "AppKit", kind = "framework")]
    unsafe extern "C" {
        static NSFontAttributeName: id;
        #[cfg(test)]
        static NSForegroundColorAttributeName: id;
        #[cfg(test)]
        static NSAppearanceNameDarkAqua: id;
    }

    pub fn apply(titles: &[Entry]) {
        let Ok(font) = font::new_from_buffer(include_bytes!("../../../assets/fonts/Bravura.otf"))
        else {
            crate::diagnostics::log("No se pudo cargar Bravura para los menús");
            return;
        };
        let font = font.clone_with_font_size(f64::from(MUSIC_SIZE));
        let metrics = MusicFont::new();
        // SAFETY: install runs on the AppKit thread immediately after GPUI's
        // synchronous set_menus. All menu objects are borrowed; AppKit retains
        // the attributed titles, attachments and images before this pool drains.
        unsafe {
            let pool = NSAutoreleasePool::new(nil);
            let app: id = msg_send![class!(NSApplication), sharedApplication];
            let main_menu: id = msg_send![app, mainMenu];
            let text_font: id = msg_send![class!(NSFont), menuFontOfSize: 0.0f64];
            for entry in titles {
                let mut menu = main_menu;
                let mut item = nil;
                for (depth, index) in entry.path.iter().enumerate() {
                    if menu == nil {
                        break;
                    }
                    item = msg_send![menu, itemAtIndex: *index as isize];
                    if depth + 1 < entry.path.len() {
                        menu = msg_send![item, submenu];
                    }
                }
                if item == nil || menu == nil {
                    continue;
                }
                let styled = attributed_title(&entry.title, text_font, &font, &metrics, item);
                let _: () = msg_send![item, setAttributedTitle: styled];
                let label = NSString::alloc(nil)
                    .init_str(&entry.accessible_name)
                    .autorelease();
                let _: () = msg_send![item, setAccessibilityLabel: label];
            }
            let _: () = msg_send![pool, drain];
        }
    }

    /// Bravura's global ascent/descent is four em. Inline attachments let AppKit
    /// lay out the actual glyph ink with the native menu font's line metrics.
    /// The symbols themselves are drawn from the exact bundled score font.
    unsafe fn attributed_title(
        title: &Title,
        text_font: id,
        font: &CTFont,
        metrics: &MusicFont,
        item: id,
    ) -> id {
        unsafe {
            let text = NSString::alloc(nil).init_str(&title.text).autorelease();
            let styled: id = msg_send![class!(NSMutableAttributedString), alloc];
            let styled: id = msg_send![styled, initWithString: text];
            let styled = styled.autorelease();
            let units: Vec<_> = title.text.encode_utf16().collect();
            for range in title.music.iter().rev() {
                let symbol = String::from_utf16(&units[range.clone()]).expect("SMuFL text");
                let ink = metrics.bounds(&symbol, MUSIC_SIZE);
                let image_size = NSSize::new(
                    f64::from(f32::from(ink.size.width)).ceil() + 2.,
                    f64::from(f32::from(ink.size.height)).ceil() + 2.,
                );
                let mut glyphs = vec![0; range.len()];
                assert!(font.get_glyphs_for_characters(
                    units[range.clone()].as_ptr(),
                    glyphs.as_mut_ptr(),
                    glyphs.len() as isize
                ));
                let mut advances = vec![CGSize::new(0., 0.); glyphs.len()];
                font.get_advances_for_glyphs(
                    core_text::font_descriptor::kCTFontOrientationHorizontal,
                    glyphs.as_ptr(),
                    advances.as_mut_ptr(),
                    glyphs.len() as isize,
                );
                let mut x = 1. - f64::from(f32::from(ink.origin.x));
                let y = 1. + f64::from(f32::from(ink.size.height + ink.origin.y));
                let positions: Vec<_> = advances
                    .iter()
                    .map(|advance| {
                        let p = CGPoint::new(x, y);
                        x += advance.width;
                        p
                    })
                    .collect();
                let weak_item = WeakPtr::new(item);
                let drawing_font = font.clone();
                let drawing = ConcreteBlock::new(move |_: NSRect| -> BOOL {
                    let item = weak_item.load();
                    let on_main: BOOL = msg_send![class!(NSThread), isMainThread];
                    let color: id = if *item != nil && on_main == YES {
                        let enabled: BOOL = msg_send![*item, isEnabled];
                        let menu: id = msg_send![*item, menu];
                        let highlighted: id = msg_send![menu, highlightedItem];
                        if enabled == NO {
                            msg_send![class!(NSColor), disabledControlTextColor]
                        } else if highlighted == *item {
                            msg_send![class!(NSColor), selectedMenuItemTextColor]
                        } else {
                            msg_send![class!(NSColor), controlTextColor]
                        }
                    } else {
                        msg_send![class!(NSColor), controlTextColor]
                    };
                    let _: () = msg_send![color, setFill];
                    let graphics: id = msg_send![class!(NSGraphicsContext), currentContext];
                    let raw: *mut core_graphics::sys::CGContext = msg_send![graphics, CGContext];
                    let context = CGContext::from_existing_context_ptr(raw);
                    drawing_font.draw_glyphs(&glyphs, &positions, context);
                    YES
                })
                .copy();
                let image: id = msg_send![class!(NSImage), imageWithSize: image_size flipped: NO drawingHandler: &*drawing];
                // Redraw at the display's resolution and resolve color for the
                // current appearance, disabled state and highlighted menu item.
                let _: () = msg_send![image, setCacheMode: cocoa::appkit::NSImageCacheMode::NSImageCacheNever as usize];
                let attachment: id = msg_send![class!(NSTextAttachment), new];
                let attachment = attachment.autorelease();
                let _: () = msg_send![attachment, setImage: image];
                let x_height: f64 = msg_send![text_font, xHeight];
                let bounds = NSRect::new(
                    NSPoint::new(0., (x_height - image_size.height) / 2.),
                    image_size,
                );
                let _: () = msg_send![attachment, setBounds: bounds];
                let replacement: id = msg_send![class!(NSAttributedString), attributedStringWithAttachment: attachment];
                let range = NSRange::new(range.start as u64, range.len() as u64);
                let _: () = msg_send![styled, replaceCharactersInRange: range withAttributedString: replacement];
            }
            let length: u64 = msg_send![styled, length];
            let _: () = msg_send![styled, addAttribute: NSFontAttributeName value: text_font range: NSRange::new(0, length)];
            styled
        }
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::shortcuts::{ChooseDynamic, ChooseKeySignature, MenuRun};
        use cocoa::appkit::NSImage;
        use gpui::{Menu, MenuItem};
        use score_core::{Dynamic, KeySignature};
        use score_keymap::id as command;

        #[test]
        fn native_bravura_titles_fit_menu_rows() {
            let font = font::new_from_buffer(include_bytes!("../../../assets/fonts/Bravura.otf"))
                .unwrap()
                .clone_with_font_size(f64::from(MUSIC_SIZE));
            assert_eq!(font.postscript_name(), "Bravura");
            let metrics = MusicFont::new();
            let entries = super::super::titles::collect(&[Menu {
                name: "Test".into(),
                items: Dynamic::ALL
                    .into_iter()
                    .map(|mark| MenuItem::action(mark.label(), ChooseDynamic(Some(mark))))
                    .chain([
                        MenuItem::action("Mi♭ mayor", ChooseKeySignature(KeySignature::default())),
                        MenuItem::action("Fa♯ mayor", ChooseKeySignature(KeySignature::default())),
                        MenuItem::action("Sostenido  (8)", MenuRun(command::SHARP)),
                        MenuItem::action("Crescendo  (H)", MenuRun(command::CRESCENDO)),
                        MenuItem::action("Calderón", MenuRun(command::FERMATA)),
                    ])
                    .collect(),
            }]);
            // SAFETY: these are offscreen AppKit text/image objects, with an
            // explicit pool. No windows or application menu state are touched.
            unsafe {
                let pool = NSAutoreleasePool::new(nil);
                let text_font: id = msg_send![class!(NSFont), menuFontOfSize: 0.0f64];
                let preview_path = std::env::var("LEVARE_MENU_PREVIEW")
                    .ok()
                    .or_else(|| std::env::var("PARTITURA_MENU_PREVIEW").ok());
                let image = NSImage::alloc(nil)
                    .initWithSize_(NSSize::new(720., 330.))
                    .autorelease();
                for (index, entry) in entries.iter().enumerate() {
                    let styled = attributed_title(&entry.title, text_font, &font, &metrics, nil);
                    let bounds: NSSize = msg_send![styled, size];
                    assert!(
                        bounds.width > 0. && bounds.height <= 22.,
                        "{}: {}x{}",
                        entry.accessible_name,
                        bounds.width,
                        bounds.height
                    );
                    if preview_path.is_some() {
                        image.lockFocus();
                        if index == 0 {
                            let white: id = msg_send![class!(NSColor), whiteColor];
                            let _: () = msg_send![white, setFill];
                            let _: () = msg_send![class!(NSBezierPath), fillRect: NSRect::new(NSPoint::new(0., 0.), NSSize::new(720., 330.))];
                        }
                        let _: () = msg_send![styled, drawAtPoint: NSPoint::new(20., 300. - index as f64 * 24.)];

                        if index == 0 {
                            let black: id = msg_send![class!(NSColor), blackColor];
                            let _: () = msg_send![black, setFill];
                            let _: () = msg_send![class!(NSBezierPath), fillRect: NSRect::new(NSPoint::new(360., 0.), NSSize::new(360., 330.))];
                        }
                        let old: id = msg_send![class!(NSAppearance), currentAppearance];
                        let old = objc::rc::StrongPtr::retain(old);
                        let dark: id = msg_send![class!(NSAppearance), appearanceNamed: NSAppearanceNameDarkAqua];
                        let _: () = msg_send![class!(NSAppearance), setCurrentAppearance: dark];
                        let color: id = msg_send![class!(NSColor), controlTextColor];
                        let length: u64 = msg_send![styled, length];
                        let _: () = msg_send![styled, addAttribute: NSForegroundColorAttributeName value: color range: NSRange::new(0,length)];
                        let _: () = msg_send![styled, drawAtPoint: NSPoint::new(380., 300. - index as f64 * 24.)];
                        let _: () = msg_send![class!(NSAppearance), setCurrentAppearance: *old];
                        image.unlockFocus();
                    }
                }
                if let Some(path) = preview_path {
                    let tiff: id = msg_send![image, TIFFRepresentation];
                    let bitmap: id = msg_send![class!(NSBitmapImageRep), imageRepWithData: tiff];
                    let properties: id = msg_send![class!(NSDictionary), dictionary];
                    let png: id =
                        msg_send![bitmap, representationUsingType: 4u64 properties: properties];
                    let path = NSString::alloc(nil).init_str(&path).autorelease();
                    let _: bool = msg_send![png, writeToFile: path atomically: YES];
                }
                let _: () = msg_send![pool, drain];
            }
        }
    }
}

#[cfg(any(target_os = "macos", test))]
mod titles {
    use crate::shortcuts::{ChooseDynamic, MenuRun, Run};
    use gpui::{Action, Menu, MenuItem};
    use score_keymap::{CommandId, id};
    use std::ops::Range;

    pub struct Title {
        pub text: String,
        /// NSAttributedString ranges count UTF-16 code units, not UTF-8 bytes.
        pub music: Vec<Range<usize>>,
    }

    // Elsewhere only the tests build entries, and they check the paths.
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    pub struct Entry {
        pub path: Vec<usize>,
        pub title: Title,
        pub accessible_name: String,
    }

    fn command_symbol(command: CommandId) -> Option<&'static str> {
        if let Some(mark) = score_toolbar::marks::kind(command) {
            return mark.symbol();
        }
        Some(match command {
            id::NATURAL => "\u{e261}",
            id::SHARP => "\u{e262}",
            id::FLAT => "\u{e260}",
            id::DOT => "\u{e1e7}",
            id::TRIPLET => "\u{e883}",
            _ => return None,
        })
    }

    fn title(name: &str, action: &dyn Action) -> Option<Title> {
        let dynamic = action
            .as_any()
            .downcast_ref::<ChooseDynamic>()
            .and_then(|choice| choice.0);
        let command = action
            .as_any()
            .downcast_ref::<Run>()
            .map(|run| run.0)
            .or_else(|| action.as_any().downcast_ref::<MenuRun>().map(|run| run.0));
        let source = if let Some(dynamic) = dynamic {
            dynamic.glyph().to_owned()
        } else if let Some(symbol) = command.and_then(command_symbol) {
            format!("{symbol}\u{2002}{name}")
        } else {
            name.to_owned()
        };
        let mut text = String::new();
        let mut music: Vec<Range<usize>> = Vec::new();
        let mut offset = 0;
        for character in source.chars() {
            let glyph = crate::music_font::bravura_character(character);
            let character = glyph.unwrap_or(character);
            let end = offset + character.len_utf16();
            if glyph.is_some() {
                if let Some(last) = music.last_mut().filter(|last| last.end == offset) {
                    last.end = end;
                } else {
                    music.push(offset..end);
                }
            }
            text.push(character);
            offset = end;
        }
        (!music.is_empty()).then_some(Title { text, music })
    }

    pub fn collect(menus: &[Menu]) -> Vec<Entry> {
        fn visit(items: &[MenuItem], path: &mut Vec<usize>, entries: &mut Vec<Entry>) {
            for (index, item) in items.iter().enumerate() {
                path.push(index);
                match item {
                    MenuItem::Action { name, action, .. } => {
                        if let Some(title) = title(name, action.as_ref()) {
                            entries.push(Entry {
                                path: path.clone(),
                                title,
                                accessible_name: name.to_string(),
                            });
                        }
                    }
                    MenuItem::Submenu(menu) => visit(&menu.items, path, entries),
                    _ => {}
                }
                path.pop();
            }
        }
        let mut entries = Vec::new();
        for (index, menu) in menus.iter().enumerate() {
            visit(&menu.items, &mut vec![index], &mut entries);
        }
        entries
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::shortcuts::ChooseKeySignature;
        use score_core::{Dynamic, KeySignature};

        #[test]
        fn dynamics_use_score_glyphs_and_removal_stays_text() {
            for mark in Dynamic::ALL {
                let title = title(mark.label(), &ChooseDynamic(Some(mark))).unwrap();
                assert_eq!(title.text, mark.glyph());
                assert_eq!(title.music, vec![0..mark.glyph().encode_utf16().count()]);
            }
            assert!(title("Quitar dinámica", &ChooseDynamic(None)).is_none());
        }

        #[test]
        fn every_catalog_symbol_is_styled_as_music_in_native_menus() {
            let font = crate::music_font::MusicFont::new();
            for mark in score_core::MarkKind::ALL {
                let Some(symbol) = mark.symbol() else {
                    continue;
                };
                let command = score_toolbar::marks::command(mark);
                let styled = title(mark.name(), &MenuRun(command)).unwrap();
                assert_eq!(
                    styled.music[0],
                    0..symbol.encode_utf16().count(),
                    "{}",
                    mark.name()
                );
                assert!(font.covers(symbol));
            }
            let fermata = title("Calderón", &MenuRun(id::FERMATA)).unwrap();
            assert_eq!(fermata.music, vec![0..1]);
        }

        #[test]
        fn mixed_titles_keep_names_and_shortcuts_with_utf16_ranges() {
            let title =
                title("🎹 Mi♭ mayor", &ChooseKeySignature(KeySignature::default())).unwrap();
            assert_eq!(title.text, "🎹 Mi\u{e260} mayor");
            assert_eq!(title.music, vec![5..6]);
            let title = super::title("Sostenido  (8)", &MenuRun(id::SHARP)).unwrap();
            assert_eq!(title.text, "\u{e262}\u{2002}Sostenido  (8)");
            assert_eq!(title.music, vec![0..1]);
            assert!(super::title("Guardar", &Run(id::SAVE)).is_none());
            assert!(super::title("Guardar  (\u{f8ff})", &Run(id::SAVE)).is_none());
            let custom = super::title("Sostenido  (\u{f8ff})", &MenuRun(id::SHARP)).unwrap();
            assert_eq!(custom.music, vec![0..1]);
        }

        #[test]
        fn nested_menu_paths_preserve_separators_and_unstyled_items() {
            let menus = vec![Menu {
                name: "Expresión".into(),
                items: vec![
                    MenuItem::separator(),
                    MenuItem::submenu(Menu {
                        name: "Dinámicas".into(),
                        items: vec![MenuItem::action("f", ChooseDynamic(Some(Dynamic::F)))],
                    }),
                ],
            }];
            let entries = collect(&menus);
            assert_eq!(entries.len(), 1);
            assert_eq!(entries[0].path, vec![0, 1, 0]);
        }
    }
}
