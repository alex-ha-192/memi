//! The first time Memi opens on a computer with nothing in it: has this person used Memi before?
//! If so, they sign in to Google and another of their devices lets this computer in. If not, they
//! choose between syncing with Google and keeping everything on this computer. Both answers look
//! the same, nothing is chosen for them, and it's never asked again (Sync stays in Settings).

use super::app::{MemiView, Overlay};
use super::theme::Colors;
use super::widgets::*;
use gpui::prelude::FluentBuilder;
use gpui::*;

#[derive(Clone, Copy, PartialEq)]
pub enum Welcome {
    /// "Have you used Memi before?"
    Question,
    /// "Use Memi on more than one device?"
    Choice,
}

impl MemiView {
    /// Settles whether to show the first-launch questions: only on a computer with no people and
    /// sync off that hasn't answered them. Every install from before 0.6.2 is marked as answered.
    pub fn settle_welcome(&mut self, cx: &mut Context<Self>) {
        if self.prefs.welcomed { return; }
        if self.people.is_empty() && !self.sync_ui.settings.enabled {
            self.welcome = Some(Welcome::Question);
        } else {
            self.finish_welcome(cx);
        }
    }

    fn finish_welcome(&mut self, cx: &mut Context<Self>) {
        self.welcome = None;
        let mut p = self.prefs.clone();
        p.welcomed = true;
        self.set_prefs(p, cx);
    }

    /// Opens Sync to pick where it goes (Google Drive, WebDAV or a folder). Connecting there shows
    /// the code to compare if another device has the key.
    fn welcome_sync(&mut self, cx: &mut Context<Self>) {
        self.finish_welcome(cx);
        self.overlay = Some(Overlay::Sync);
        cx.notify();
    }

    pub fn welcome_view(&mut self, step: Welcome, c: &Colors, cx: &mut Context<Self>) -> Div {
        let (title, intro) = match step {
            Welcome::Question => ("Have you used Memi before?", "If Memi is on another of your devices, this computer can pick up where it left off."),
            Welcome::Choice => ("Hi, I'm Memi", "I keep the little things about the people you care about: gift ideas, plans, and what they mentioned last time."),
        };
        let mut choices = div().flex().flex_col().gap(px(12.));
        match step {
            Welcome::Question => {
                choices = choices
                    .child(choice("yes", "monitor-smartphone", c.tile(3), c.tile_ink(3), "Yes, on another device",
                        "Connect to the same place it syncs to (Google Drive, WebDAV or a folder), then allow this computer from the other device.", c)
                        .on_click(cx.listener(|this, _, _, cx| this.welcome_sync(cx))))
                    .child(choice("new", "sparkles", c.gift(), c.gift_ink(), "No, I'm new",
                        "Start fresh. Next, you choose whether to sync or keep everything on this computer.", c)
                        .on_click(cx.listener(|this, _, _, cx| { this.welcome = Some(Welcome::Choice); cx.notify(); })));
            }
            Welcome::Choice => {
                choices = choices
                    .child(div().pt(px(4.)).child(caption("USE MEMI ON MORE THAN ONE DEVICE?", c.muted)))
                    .child(choice("sync", "cloud", c.tile(3), c.tile_ink(3), "Turn on sync",
                        "Your computer, phone and browser stay in step, through Google Drive, WebDAV or a folder. Encrypted here before it leaves.", c)
                        .on_click(cx.listener(|this, _, _, cx| this.welcome_sync(cx))))
                    .child(choice("here", "monitor", c.tile(1), c.tile_ink(1), "Just this computer",
                        "Everything stays here. You can turn on sync later in Settings.", c)
                        .on_click(cx.listener(|this, _, _, cx| { this.finish_welcome(cx); this.check_reminders(cx); })));
            }
        }
        div().size_full().flex().child(div().id("welcome").size_full().flex().items_center().justify_center().bg(c.bg).overflow_y_scroll().child(
            div().w(px(560.)).max_w(relative(0.92)).my(px(32.)).rounded(px(32.)).bg(c.card).p(px(36.)).flex().flex_col().gap(px(18.))
                .child(div().flex().justify_center().child(
                    div().size(px(132.)).rounded_full().bg(c.tile(3)).flex().items_center().justify_center()
                        .child(img("mascot.png").size(px(94.))),
                ))
                .child(div().flex().flex_col().items_center().gap(px(8.))
                    .child(t(title, 28., FontWeight::EXTRA_BOLD, c.ink))
                    .child(div().text_center().child(body(intro, c.muted))))
                .child(choices)
                .when(step == Welcome::Choice, |d| d.child(
                    div().id("back").cursor_pointer().child(label("← Back", c.muted))
                        .on_click(cx.listener(|this, _, _, cx| { this.welcome = Some(Welcome::Question); cx.notify(); })),
                ))
                .child(div().text_center().child(caption("Memi has no servers and no accounts, and nobody else gets a copy, including us.", c.muted)))
                .child(div().flex().justify_center().child(
                    div().id("welcome-privacy").cursor_pointer().child(label("How sync handles your data", c.tile_ink(3)))
                        .on_click(|_, _, _| { let _ = webbrowser::open("https://yourmemi.app/privacy/"); }),
                )),
        ))
    }
}

/// One of two answers, drawn exactly alike so neither is pushed.
fn choice(id: &'static str, icon_name: &str, chip: Hsla, ink: Hsla, title: &str, text: &str, c: &Colors) -> Stateful<Div> {
    div().id(id).role(Role::Button).aria_label(title.to_string()).aria_description(text.to_string()).w_full().min_w(px(0.)).rounded(px(22.)).bg(c.bg).border_1().border_color(c.line).p(px(16.))
        .flex().gap(px(14.)).items_start()
        .cursor_pointer().hover(|s| s.border_color(c.muted))
        .child(div().size(px(44.)).flex_none().rounded(px(14.)).bg(chip).flex().items_center().justify_center()
            .child(icon(icon_name, 22., ink)))
        .child(div().flex_1().min_w(px(0.)).flex().flex_col().gap(px(4.))
            .child(t(title.to_string(), 17., FontWeight::BOLD, c.ink))
            .child(body(text.to_string(), c.muted)))
}
