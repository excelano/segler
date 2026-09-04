//! What the desktop says about light and dark, where the window toolkit will not.
//
// Copied from excelano/slipcase-desktop, where it was measured, and unchanged
// apart from this note: the defect it answers is in `winit` on Linux and reaches
// every eframe application alike.
//
//
// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)
//
// **This module exists for Linux and does nothing anywhere else.** `winit`
// answers `system_theme()` on the other two platforms — the Windows arm calls
// `should_use_dark_mode()` and the macOS arm reads `NSApplication`'s
// `effectiveAppearance` — and on Linux it returns `None` unconditionally
// (`winit-0.30.13`, `src/platform_impl/linux/mod.rs:909`, a body that is the
// word and nothing else). `egui-winit` passes that `Option` through untouched
// and egui falls back to `Theme::Dark`, so before this module every Linux user
// saw the dark card and no desktop setting could reach them.
//
// Measured 2026-08-28 before it was written, because the comfortable
// explanations had to be ruled out first. With GNOME set to Light the card drew
// dark; with `color-scheme` forced to `prefer-light`, so the portal answered
// `uint32 2` rather than `0`, the card drew dark again *and the window's own
// titlebar turned light in the same screenshot*. One window, two halves,
// disagreeing — which is what rules out the desktop having failed to say what
// it wanted. The run and the pixel samples are in `git log`.
//
// The titlebar's answer comes from `sctk-adwaita`, which spawns `dbus-send` and
// greps its output for `uint32 1` under a 100ms timeout. That is the second
// implementation this window would have had, and it is why this one asks the
// portal properly rather than shelling out beside it.

/// Which way a `color-scheme` answer points.
///
/// A named type rather than `egui::ThemePreference` so that the mapping below
/// can be tested without a window, and so that the one case that is a judgement
/// rather than a reading — zero — is visible at the point it is decided.
#[cfg(target_os = "linux")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scheme {
    Dark,
    Light,
}

/// What the desktop portal's `color-scheme` means, including the value that is
/// a decision rather than a reading.
///
/// The XDG specification defines three: 0 no preference, 1 prefer dark, 2
/// prefer light. **Zero is treated as light, and that is the whole of what makes
/// this module work on GNOME.** GNOME's Settings offers Light and Dark and
/// spells them `default` and `prefer-dark`, so choosing Light sets
/// `color-scheme` to `default` and the portal answers 0 — never 2. Measured on
/// GNOME 48.7: `prefer-light` has to be set by hand through `gsettings` and no
/// part of the interface will do it.
///
/// So reading 0 as *leave it dark* would mean every GNOME user who chose Light
/// still got the dark card, which is the defect this module was written for
/// wearing a different hat. Reading it as light is also what GTK does with
/// `ADW_COLOR_SCHEME_DEFAULT`, so an application that follows this rule looks
/// like every other application on that desktop rather than like the one
/// exception.
///
/// The cost is a desktop that is genuinely dark while declaring no preference,
/// where this now draws light. That desktop's GTK applications are drawing
/// light too, so the answer is at least consistent with its neighbours.
#[cfg(target_os = "linux")]
#[must_use]
pub fn scheme_of(color_scheme: u32) -> Scheme {
    match color_scheme {
        1 => Scheme::Dark,
        _ => Scheme::Light,
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use super::{scheme_of, Scheme};

    const PORTAL: &str = "org.freedesktop.portal.Desktop";
    const PATH: &str = "/org/freedesktop/portal/desktop";
    const SETTINGS: &str = "org.freedesktop.portal.Settings";
    const NAMESPACE: &str = "org.freedesktop.appearance";
    const KEY: &str = "color-scheme";

    /// The portal answers `Read` with a variant inside a variant, and `ReadOne`
    /// with one. Rather than choose a method by portal version — the older one
    /// does not have `ReadOne` and the newer one deprecates `Read` — this
    /// unwraps whatever arrived until a number falls out.
    ///
    /// Written as a loop because the nesting is a property of the wire format
    /// and not of this application, and a `u32` at any depth means the same
    /// thing.
    fn number_inside(value: &zbus::zvariant::Value<'_>) -> Option<u32> {
        let mut here = value;
        for _ in 0..4 {
            match here {
                zbus::zvariant::Value::U32(n) => return Some(*n),
                zbus::zvariant::Value::Value(inner) => here = inner,
                _ => return None,
            }
        }
        None
    }

    fn proxy(
        connection: &zbus::blocking::Connection,
    ) -> Result<zbus::blocking::Proxy<'static>, zbus::Error> {
        zbus::blocking::Proxy::new(connection, PORTAL, PATH, SETTINGS)
    }

    /// Ask once. `None` where the question could not be put at all — no session
    /// bus, no portal, or a portal that does not carry the appearance
    /// namespace.
    ///
    /// **`None` is not a third answer and must not be turned into one.** It
    /// means nothing was learned, and the caller leaves egui's own fallback
    /// alone rather than choosing a theme on no evidence. A machine with no
    /// portal therefore behaves exactly as it did before this module existed.
    pub fn ask() -> Option<Scheme> {
        let connection = zbus::blocking::Connection::session().ok()?;
        let proxy = proxy(&connection).ok()?;
        let value: zbus::zvariant::OwnedValue = proxy.call("Read", &(NAMESPACE, KEY)).ok()?;
        number_inside(&value).map(scheme_of)
    }

    /// Watch for the setting changing while the window is open, on a thread of
    /// its own.
    ///
    /// The other two platforms get this for free: `winit` delivers
    /// `WindowEvent::ThemeChanged` and `egui-winit` turns it into a new
    /// `system_theme` without anybody here being involved. Following the
    /// setting only at startup would leave Linux the one platform where the
    /// answer goes stale the moment a person changes their mind, which is not
    /// what *respects the setting* means anywhere else.
    ///
    /// The thread is detached and never joined. It holds a D-Bus connection and
    /// a clone of the context for as long as the process runs, and there is
    /// nothing for it to clean up: the process exiting closes the socket.
    pub fn watch(ctx: &eframe::egui::Context, mut apply: impl FnMut(Scheme) + Send + 'static) {
        let ctx = ctx.clone();
        std::thread::Builder::new()
            .name("slipcase-theme".to_owned())
            .spawn(move || {
                let Ok(connection) = zbus::blocking::Connection::session() else {
                    return;
                };
                let Ok(proxy) = proxy(&connection) else {
                    return;
                };
                let Ok(changes) = proxy.receive_signal("SettingChanged") else {
                    return;
                };
                for message in changes {
                    // The signal carries every setting the portal has, so the
                    // namespace and key are checked rather than assumed. A
                    // desktop emits these for font sizes and accent colours
                    // too, and repainting the window for an accent change
                    // would be a wakeup for nothing.
                    // Bound before it is read: the body borrows the message,
                    // so deserializing inline drops it at the end of the
                    // statement and leaves the value pointing at nothing.
                    let body = message.body();
                    let Ok((namespace, key, value)) =
                        body.deserialize::<(String, String, zbus::zvariant::Value<'_>)>()
                    else {
                        continue;
                    };
                    if namespace != NAMESPACE || key != KEY {
                        continue;
                    }
                    if let Some(number) = number_inside(&value) {
                        apply(scheme_of(number));
                        // The thread has no frame of its own, so the window is
                        // asked for one. Without this the new theme sits in
                        // egui's options until something else happens to cause
                        // a repaint — a pointer moving over the window, most
                        // likely — which looks exactly like the setting having
                        // been ignored.
                        ctx.request_repaint();
                    }
                }
            })
            .ok();
    }
}

/// Follow the desktop's light and dark setting, where the window toolkit does
/// not do it for us.
///
/// Called from the creation closure, where the context exists. On every
/// platform but Linux this is empty: `winit` reports the system theme there and
/// egui is already following it, and calling `set_theme` would replace a live
/// answer with a pinned one.
#[cfg(target_os = "linux")]
pub fn follow(ctx: &eframe::egui::Context) {
    use eframe::egui::ThemePreference;

    fn preference(scheme: Scheme) -> ThemePreference {
        match scheme {
            Scheme::Dark => ThemePreference::Dark,
            Scheme::Light => ThemePreference::Light,
        }
    }

    // `set_theme` and not `Options::system_theme`, because the second is what
    // egui fills in from `winit` and this platform's whole problem is that
    // nothing fills it in. Pinning the preference is the honest description of
    // what is happening: this module, and not the toolkit, is deciding.
    if let Some(scheme) = linux::ask() {
        ctx.set_theme(preference(scheme));
    }

    let watcher = ctx.clone();
    linux::watch(ctx, move |scheme| {
        watcher.set_theme(preference(scheme));
    });
}

#[cfg(not(target_os = "linux"))]
pub fn follow(_ctx: &eframe::egui::Context) {}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::{scheme_of, Scheme};

    /// Would catch the defect this module was written for: reading the
    /// portal's *no preference* as a reason to leave the window dark.
    ///
    /// GNOME's Settings spells Light as `color-scheme=default`, which the
    /// portal reports as 0 and never as 2, so a mapping that treats 0 as
    /// anything but light gives every GNOME user who chose Light the dark card
    /// — which is the state this module replaced.
    #[test]
    fn no_preference_is_light_because_that_is_what_gnome_sends_for_light() {
        assert_eq!(scheme_of(0), Scheme::Light);
    }

    /// Would catch the mapping being inverted, which is the one way this can
    /// be wrong while still appearing to work: the theme would change when the
    /// setting changed, and be wrong both times.
    #[test]
    fn one_is_dark_and_two_is_light() {
        assert_eq!(scheme_of(1), Scheme::Dark);
        assert_eq!(scheme_of(2), Scheme::Light);
    }

    /// Would catch a value outside the specification's three landing on dark.
    ///
    /// Light is the same answer 0 gets and for the same reason: a value this
    /// build does not understand is not evidence that a person wants a dark
    /// window, and the desktops that send one will have their other
    /// applications drawing light.
    #[test]
    fn a_value_the_specification_does_not_define_is_light() {
        for unknown in [3, 4, 99, u32::MAX] {
            assert_eq!(
                scheme_of(unknown),
                Scheme::Light,
                "color-scheme {unknown} should not darken the window"
            );
        }
    }
}
