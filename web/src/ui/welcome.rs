//! The first time Memi opens in a browser with nothing in it: has this person used Memi before?
//! If so, they sign in to Google and another of their devices lets this browser in. If not, they
//! choose between syncing with Google and keeping everything in this browser. Both answers look
//! the same, nothing is chosen for them, and it's never asked again (Sync stays in Settings).
//! Google's sign-in script still loads only if they choose to sign in.

use super::widgets::Icon;
use crate::state::{app, Overlay, Welcome};
use leptos::prelude::*;

#[component]
pub fn WelcomeView() -> impl IntoView {
    let a = app();
    let finish = move || {
        a.welcome.set(None);
        a.update_settings(|s| s.welcomed = true);
    };
    // Sync opens to pick where it goes (Google Drive or WebDAV); connecting there shows the code
    // to compare if another device has the key.
    let sign_in = move |_| {
        finish();
        a.overlay.set(Some(Overlay::Sync));
    };
    let step = move || a.welcome.get().unwrap_or(Welcome::Question);
    view! {
        <div class="first-launch">
            <div class="first-launch-card">
                <div class="first-launch-memi"><img src="/memi.png" alt="Memi the elephant" width="96" height="96" /></div>
                <Show when=move || step() == Welcome::Question fallback=move || view! {
                    <h1>"Hi, I'm Memi"</h1>
                    <p class="muted centered">"I keep the little things about the people you care about: gift ideas, plans, and what they mentioned last time."</p>
                    <p class="eyebrow muted">"USE MEMI ON MORE THAN ONE DEVICE?"</p>
                    <button class="choice" on:click=sign_in>
                        <span class="chip-icon c3"><Icon name="cloud" size=22. /></span>
                        <span><strong>"Turn on sync"</strong><span class="muted">"Your browser, phone and computer stay in step, through Google Drive or WebDAV. It's encrypted in this browser before it leaves."</span></span>
                    </button>
                    <button class="choice" on:click=move |_| finish()>
                        <span class="chip-icon c1"><Icon name="monitor" size=22. /></span>
                        <span><strong>"Just this browser"</strong><span class="muted">"Everything stays here. You can turn on sync later in Settings."</span></span>
                    </button>
                    <button class="back" on:click=move |_| a.welcome.set(Some(Welcome::Question))>"← Back"</button>
                }>
                    <h1>"Have you used Memi before?"</h1>
                    <p class="muted centered">"If Memi is on another of your devices, this browser can pick up where it left off."</p>
                    <button class="choice" on:click=sign_in>
                        <span class="chip-icon c3"><Icon name="devices" size=22. /></span>
                        <span><strong>"Yes, on another device"</strong><span class="muted">"Connect to the same place it syncs to, then allow this browser from the other device."</span></span>
                    </button>
                    <button class="choice" on:click=move |_| a.welcome.set(Some(Welcome::Choice))>
                        <span class="chip-icon c0"><Icon name="sparkles" size=22. /></span>
                        <span><strong>"No, I'm new"</strong><span class="muted">"Start fresh. Next, you choose whether to sync or keep everything in this browser."</span></span>
                    </button>
                </Show>
                <p class="muted centered fine">"Memi has no servers and no accounts, and nobody else gets a copy, including us."</p>
                <a class="centered link accent" href="https://yourmemi.app/privacy/" target="_blank" rel="noopener">"How sync handles your data"</a>
            </div>
        </div>
    }
}
